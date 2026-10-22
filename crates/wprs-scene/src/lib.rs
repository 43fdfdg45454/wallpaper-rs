mod effect;
mod layer;
mod perf;
mod property;
mod rules;
mod timeline;
mod validate;

pub use effect::{EFFECTS_WGSL, Effect};
pub use layer::{Blend, Emitter, Layer, LayerKind, Transform};
pub use perf::{Cost, Quality};
pub use property::{Property, PropertyKind, Value};
pub use timeline::{Easing, Repeat, Timeline};
pub use validate::Issue;

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA: &str = "wallpaper-rs.scene/1";
pub const MAX_SIZE: u32 = 16_384;
pub const MAX_LAYERS: usize = 64;
pub const MAX_EFFECTS: usize = 16;
pub const MAX_STOPS: usize = 16;
pub const MAX_PARTICLES: u32 = 100_000;
pub const MAX_RATE: f32 = 5_000.0;
pub const MAX_KEYFRAMES: usize = 1_024;
pub const MAX_TIME: f32 = 3_600.0;

#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    #[error("invalid scene: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0} is not a bindable layer field of matching type")]
    Binding(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Color(pub [f32; 4]);

impl Default for Color {
    fn default() -> Self {
        Color([0.0, 0.0, 0.0, 1.0])
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub schema: String,
    pub size: (u32, u32),
    #[serde(default)]
    pub background: Color,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timelines: Vec<Timeline>,
}

impl Scene {
    pub fn new(size: (u32, u32)) -> Scene {
        let (layers, properties, timelines) = Default::default();
        Scene { schema: SCHEMA.into(), size, background: Color::default(), layers, properties, timelines }
    }

    pub fn from_json(text: &str) -> Result<Scene, SceneError> {
        Ok(serde_json::from_str(text)?)
    }

    pub fn to_json(&self) -> Result<String, SceneError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn assets(&self) -> BTreeSet<&str> {
        let layers = self.layers.iter().flat_map(|l| {
            let src = match &l.kind {
                LayerKind::Image { src } | LayerKind::Video { src } => Some(src),
                LayerKind::Particles(e) => e.texture.as_ref(),
                _ => None,
            };
            src.into_iter().chain(&l.mask)
        });
        let is_asset = |t: &String| ["/src", "/mask", "/texture"].iter().any(|f| t.ends_with(f));
        let bound =
            self.properties.iter().filter(|p| p.targets.iter().any(is_asset)).flat_map(|p| match &p.kind {
                PropertyKind::Combo { options, .. } => options.as_slice(),
                PropertyKind::Text { value } => std::slice::from_ref(value),
                _ => &[],
            });
        layers.chain(bound).map(String::as_str).filter(|s| !s.trim().is_empty()).collect()
    }
}
