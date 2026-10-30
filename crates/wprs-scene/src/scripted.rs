use crate::{Kind, LayerOverride, MAX_EFFECTS, MAX_LAYERS, MAX_PARAMS, Param, Scene, ScriptOutputs};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SPAWNED: usize = 256;
pub const MAX_WATCHED: usize = 64;
pub const MAX_SCRIPT_BYTES: usize = 1 << 20;
pub const MAX_SCRIPT_TEXT: usize = 4096;
pub const MAX_CONSOLE: usize = 20;
const GENERIC_FONTS: [&str; 3] = ["sans-serif", "serif", "monospace"];
const FONT_FILES: [&str; 5] = [".ttf", ".otf", ".ttc", ".otc", ".woff2"];

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Extent {
    Points(f32),
    Box([f32; 2]),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectControl {
    pub at: usize,
    pub enabled: Option<bool>,
    pub params: BTreeMap<String, Param>,
    pub reset: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerRead {
    pub position: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub skew: [f32; 2],
    pub origin: [f32; 2],
    pub opacity: f32,
    pub visible: bool,
    pub color: [f32; 4],
    pub parallax: [f32; 2],
    pub size: Option<Extent>,
    pub text: Option<String>,
}

fn finite<const N: usize>(v: Option<[f32; N]>, lo: f32, hi: f32) -> Option<[f32; N]> {
    v.filter(|v| v.iter().all(|c| c.is_finite())).map(|v| v.map(|c| c.clamp(lo, hi)))
}

fn one(v: Option<f32>, lo: f32, hi: f32) -> Option<f32> {
    finite(v.map(|v| [v]), lo, hi).map(|[v]| v)
}

impl LayerOverride {
    pub fn bounded(self, font: impl Fn(&str) -> bool) -> LayerOverride {
        let size = match self.size {
            Some(Extent::Points(p)) => one(Some(p), 1.0, 2048.0).map(Extent::Points),
            Some(Extent::Box(b)) => finite(Some(b), 0.0, 1e5).map(Extent::Box),
            None => None,
        };
        LayerOverride {
            position: finite(self.position, -1e5, 1e5),
            scale: finite(self.scale, -1e3, 1e3),
            rotation: one(self.rotation, -1e6, 1e6),
            opacity: one(self.opacity, 0.0, 1.0),
            visible: self.visible,
            color: finite(self.color, 0.0, 1.0),
            text: self.text.map(|t| t.chars().take(MAX_SCRIPT_TEXT).collect()),
            skew: finite(self.skew, -89.0, 89.0),
            origin: finite(self.origin, -10.0, 10.0),
            parallax: finite(self.parallax, -10.0, 10.0),
            size,
            font: self.font.filter(|f| f.len() <= 128 && font(f)),
            align: self.align,
        }
    }
}

impl Scene {
    pub fn known_font(&self, name: &str) -> bool {
        let texts = self.drawable().filter_map(|l| match &l.kind {
            Kind::Text(t) => Some(t),
            _ => None,
        });
        let named = texts.flat_map(|t| t.font.iter().chain(&t.fallback)).any(|f| f == name);
        named
            || GENERIC_FONTS.contains(&name)
            || self
                .assets
                .get(name)
                .is_some_and(|a| FONT_FILES.iter().any(|e| a.path.to_ascii_lowercase().ends_with(e)))
    }

    pub fn drawable(&self) -> impl Iterator<Item = &crate::Layer> {
        let mut stack: Vec<&crate::Layer> = self.templates.values().rev().collect();
        let spawned = std::iter::from_fn(move || {
            let layer = stack.pop()?;
            stack.extend(layer.children().iter().rev());
            Some(layer)
        });
        self.layers().chain(spawned)
    }
}

impl ScriptOutputs {
    pub fn bounded(mut self, scene: &Scene) -> ScriptOutputs {
        let room = MAX_LAYERS + MAX_SPAWNED;
        let layers = std::mem::take(&mut self.layers).into_iter().take(room);
        self.layers = layers.map(|(id, set)| (id, set.bounded(|f| scene.known_font(f)))).collect();
        let mut seen = BTreeSet::new();
        let fits = |(id, template): &(String, String)| {
            let fresh = id.len() <= 64 && !id.is_empty() && scene.layer(id).is_none();
            fresh && scene.templates.contains_key(template)
        };
        self.spawned.retain(|s| fits(s) && seen.insert(s.0.clone()));
        self.spawned.truncate(MAX_SPAWNED);
        self.order.iter_mut().for_each(|o| o.truncate(room));
        self.watch.truncate(MAX_WATCHED);
        self.effects.retain(|_, list| {
            list.retain(|c| c.at < MAX_EFFECTS && c.params.len() <= MAX_PARAMS);
            list.truncate(MAX_EFFECTS);
            !list.is_empty()
        });
        let sounds = std::mem::take(&mut self.sounds).into_iter().filter(|(id, _)| id.len() <= 64);
        self.sounds = sounds.take(crate::MAX_SOUNDS).collect();
        self.sounds.values_mut().for_each(|c| c.commands.truncate(8));
        self.console.truncate(MAX_CONSOLE);
        self.stopped.truncate(64);
        self.wake = self.wake.filter(|w| w.is_finite()).map(|w| w.clamp(0.0, 86_400.0));
        self
    }
}
