use crate::resolve::Context;
use crate::validate::{Check, Issue};
use crate::{Control, Kind, Layer, Property, Scene, Slider, Text, Value, clip};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_SOUNDS: usize = 8;
pub const VOLUME: &str = "volume";
const MAX_FADE: f32 = 60.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sound {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[schemars(extend("x-ref" = "sound"))]
    pub sound: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-ref" = "sound"))]
    pub sounds: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Order>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "s", "x-range" = [0, 600]))]
    pub pause: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "trigger"))]
    pub play_on: Option<String>,
    #[serde(default = "yes", rename = "loop")]
    pub repeat: bool,
    #[serde(default = "playing")]
    pub play: Value<bool>,
    #[serde(default = "full")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub volume: Value<f32>,
    #[serde(default = "centered")]
    #[schemars(extend("x-range" = [-1, 1]))]
    pub pan: Value<f32>,
    #[serde(default)]
    #[schemars(extend("x-unit" = "s", "x-range" = [0, 60]))]
    pub fade_in: f32,
    #[serde(default)]
    #[schemars(extend("x-unit" = "s", "x-range" = [0, 60]))]
    pub fade_out: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    #[default]
    Sequence,
    Random,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Voice {
    pub play: bool,
    pub volume: f32,
    pub pan: f32,
    pub at: f64,
}

impl Sound {
    pub fn clips(&self) -> impl Iterator<Item = &str> {
        let one = (!self.sound.is_empty()).then_some(self.sound.as_str());
        one.into_iter().chain(self.sounds.iter().map(String::as_str))
    }

    pub fn newer(&self) -> bool {
        let more = self.mode.is_some() || self.pause.is_some() || self.play_on.is_some();
        more || self.stream || !self.sounds.is_empty()
    }

    pub(crate) fn voice(&self, cx: &Context) -> Voice {
        let (volume, pan) = (self.volume.get(cx).clamp(0.0, 1.0), self.pan.get(cx).clamp(-1.0, 1.0));
        Voice { play: self.play.get(cx), volume, pan, at: 0.0 }
    }
}

impl Scene {
    pub fn sounds(&self) -> impl Iterator<Item = &Sound> {
        self.layers().filter_map(|l| match &l.kind {
            Kind::Sound(sound) => Some(sound),
            _ => None,
        })
    }

    pub fn properties(
        &self,
        declared: &BTreeMap<String, Property>,
        label: &str,
        loudness: f64,
    ) -> BTreeMap<String, Property> {
        let mut props = declared.clone();
        if self.sounds().next().is_some() {
            props.entry(VOLUME.into()).or_insert_with(|| volume_property(label, loudness));
        }
        props
    }
}

pub fn volume_property(label: &str, loudness: f64) -> Property {
    let loud = loudness.clamp(0.0, 100.0);
    let control = Control::Slider(Slider { integer: true, ..Slider::new(0.0, 100.0, Some(5.0), loud) });
    Property { label: Some(Text::Plain(label.into())), order: -1.0, ..Property::new(control) }
}

impl Check<'_> {
    pub(crate) fn sounds(&mut self) {
        let count = self.scene.sounds().count();
        if count > MAX_SOUNDS {
            self.issues.push(Issue::Sounds(count));
        }
        let fits = |p: &Property| matches!(&p.control, Control::Slider(s) if s.min >= 0.0 && s.max <= 100.0);
        if count > 0 && self.props.get(VOLUME).is_some_and(|p| !fits(p)) {
            self.issues.push(Issue::Volume);
        }
    }

    pub(crate) fn sound(&mut self, p: &str, layer: &Layer, s: &Sound) {
        if *layer != Layer::new(layer.id.clone(), layer.kind.clone()) {
            self.issues.push(Issue::Sound(p.into()));
        }
        if s.sound.is_empty() == s.sounds.is_empty() {
            self.issues.push(Issue::Clips(p.into()));
        }
        self.count(&format!("{p}/sounds"), s.sounds.len(), clip::MAX_CLIPS);
        let one = (!s.sound.is_empty()).then(|| (format!("{p}/sound"), &s.sound));
        let many = s.sounds.iter().enumerate().map(|(i, id)| (format!("{p}/sounds/{i}"), id));
        for (path, id) in one.into_iter().chain(many) {
            let asset = self.scene.assets.get(id);
            self.known(&path, "asset", id, asset.is_some());
            if asset.is_some_and(|a| !clip::playable(&a.path)) {
                self.issues.push(Issue::Type(path));
            }
            self.uses(asset.is_some_and(|a| a.extension() == "mp3"), "sound2");
        }
        self.uses(s.newer(), "sound2");
        if s.mode.is_some() && s.sounds.is_empty() {
            self.issues.push(Issue::Type(format!("{p}/mode")));
        }
        if let Some([lo, hi]) = s.pause {
            self.range(&format!("{p}/pause/0"), lo, 0.0, 600.0);
            self.range(&format!("{p}/pause/1"), hi, lo, 600.0);
        }
        if let Some(trigger) = &s.play_on {
            self.trigger(&format!("{p}/play_on"), trigger);
        }
        self.value(&format!("{p}/play"), &s.play);
        self.value(&format!("{p}/volume"), &s.volume);
        self.value(&format!("{p}/pan"), &s.pan);
        let fed =
            [s.play.binding().map(follows), s.volume.binding().map(follows), s.pan.binding().map(follows)];
        for (name, outside) in ["play", "volume", "pan"].into_iter().zip(fed) {
            if outside == Some(true) {
                self.issues.push(Issue::SoundSource(format!("{p}/{name}")));
            }
        }
        if let Value::Fixed(v) = s.volume {
            self.range(&format!("{p}/volume"), v, 0.0, 1.0);
        }
        if let Value::Fixed(v) = s.pan {
            self.range(&format!("{p}/pan"), v, -1.0, 1.0);
        }
        self.range(&format!("{p}/fade_in"), s.fade_in, 0.0, MAX_FADE);
        self.range(&format!("{p}/fade_out"), s.fade_out, 0.0, MAX_FADE);
    }
}

fn follows<T>(b: &crate::Binding<T>) -> bool {
    b.audio.is_some() || b.cursor.is_some() || b.clock.is_some() || b.script.is_some()
}

fn yes() -> bool {
    true
}

fn playing() -> Value<bool> {
    Value::Fixed(true)
}

fn full() -> Value<f32> {
    Value::Fixed(1.0)
}

fn centered() -> Value<f32> {
    Value::Fixed(0.0)
}
