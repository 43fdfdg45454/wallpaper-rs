use crate::resolve::Context;
use crate::validate::{Check, Issue};
use crate::{Control, Kind, Layer, Property, Scene, Text, Value, clip};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_SOUNDS: usize = 8;
pub const VOLUME: &str = "volume";
const MAX_FADE: f32 = 60.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sound {
    pub sound: String,
    #[serde(default = "yes", rename = "loop")]
    pub repeat: bool,
    #[serde(default = "playing")]
    pub play: Value<bool>,
    #[serde(default = "full")]
    pub volume: Value<f32>,
    #[serde(default = "centered")]
    pub pan: Value<f32>,
    #[serde(default)]
    pub fade_in: f32,
    #[serde(default)]
    pub fade_out: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Voice {
    pub play: bool,
    pub volume: f32,
    pub pan: f32,
}

impl Sound {
    pub(crate) fn voice(&self, cx: &Context) -> Voice {
        let (volume, pan) = (self.volume.get(cx).clamp(0.0, 1.0), self.pan.get(cx).clamp(-1.0, 1.0));
        Voice { play: self.play.get(cx), volume, pan }
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
    ) -> BTreeMap<String, Property> {
        let mut props = declared.clone();
        if self.sounds().next().is_some() {
            let control = Control::Slider {
                min: 0.0,
                max: 100.0,
                step: 5.0,
                default: 100.0,
                integer: true,
                unit: None,
            };
            let volume =
                Property { label: Some(Text::Plain(label.into())), order: -1, ..Property::new(control) };
            props.entry(VOLUME.into()).or_insert(volume);
        }
        props
    }
}

impl Check<'_> {
    pub(crate) fn sounds(&mut self) {
        let count = self.scene.sounds().count();
        if count > MAX_SOUNDS {
            self.issues.push(Issue::Sounds(count));
        }
        let fits = |p: &Property| matches!(p.control, Control::Slider { min, max, .. } if min >= 0.0 && max <= 100.0);
        if count > 0 && self.props.get(VOLUME).is_some_and(|p| !fits(p)) {
            self.issues.push(Issue::Volume);
        }
    }

    pub(crate) fn sound(&mut self, p: &str, layer: &Layer, s: &Sound) {
        if *layer != Layer::new(layer.id.clone(), layer.kind.clone()) {
            self.issues.push(Issue::Sound(p.into()));
        }
        let (path, asset) = (format!("{p}/sound"), self.scene.assets.get(&s.sound));
        self.known(&path, "asset", &s.sound, asset.is_some());
        if asset.is_some_and(|a| !clip::playable(&a.path)) {
            self.issues.push(Issue::Type(path));
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
