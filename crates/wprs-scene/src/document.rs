use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("{}", self.message())]
pub enum SceneError {
    Json(#[from] serde_json::Error),
    Invalid(Issue),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "crate::value::ColorRepr", into = "[f32; 4]")]
pub struct Color(pub [f32; 4]);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub schema: String,
    pub canvas: Canvas,
    #[serde(default)]
    pub clock: Clock,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<String, Asset>,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub particles: BTreeMap<String, System>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timelines: Vec<Timeline>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<Script>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub effects: BTreeMap<String, EffectFiles>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Hearing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera3d: Option<Camera3d>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub templates: BTreeMap<String, Layer>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pools: BTreeMap<String, Pool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ext: BTreeMap<String, Json>,
    #[serde(skip)]
    #[schemars(skip)]
    pub loaded: BTreeMap<String, Arc<Descriptor>>,
    #[serde(skip)]
    #[schemars(skip)]
    pub models: BTreeMap<String, Arc<ModelFile>>,
}

impl Default for Color {
    fn default() -> Self {
        Color([0.0, 0.0, 0.0, 1.0])
    }
}

impl Scene {
    pub fn new(size: [u32; 2]) -> Scene {
        let (assets, layers, particles, timelines, scripts, effects, audio, ext, loaded) = Default::default();
        let (camera3d, templates, pools, models) = Default::default();
        Scene {
            schema: SCHEMA.into(),
            canvas: Canvas::new(size),
            clock: Clock::default(),
            assets,
            layers,
            particles,
            timelines,
            scripts,
            effects,
            audio,
            camera3d,
            templates,
            pools,
            ext,
            loaded,
            models,
        }
    }

    pub fn effect(&self, id: &str) -> Option<&Descriptor> {
        self.loaded.get(id).map(|d| &**d).or_else(|| described(id).filter(|_| !self.effects.contains_key(id)))
    }

    pub fn shader(&self, id: &str) -> Option<&Descriptor> {
        self.loaded
            .get(id)
            .map(|d| &**d)
            .or_else(|| descriptor(id).filter(|_| !self.effects.contains_key(id)))
    }

    pub fn from_json(text: &str) -> Result<Scene, SceneError> {
        Ok(serde_json::from_str(text)?)
    }

    pub fn load(text: &str, props: &BTreeMap<String, Property>) -> Result<Scene, SceneError> {
        let scene = Scene::from_json(text)?;
        match scene.validate(props).into_iter().next() {
            Some(issue) => Err(SceneError::Invalid(issue)),
            None => Ok(scene),
        }
    }

    pub fn to_json(&self) -> Result<String, SceneError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        let mut stack: Vec<&Layer> = self.layers.iter().rev().collect();
        std::iter::from_fn(move || {
            let layer = stack.pop()?;
            if let Kind::Group { layers, .. } = &layer.kind {
                stack.extend(layers.iter().rev());
            }
            Some(layer)
        })
    }

    pub fn any_layer(&self, wanted: impl Fn(&Layer) -> bool) -> bool {
        fn any(layers: &[Layer], wanted: &dyn Fn(&Layer) -> bool) -> bool {
            let inner = |l: &Layer| matches!(&l.kind, Kind::Group { layers, .. } if any(layers, wanted));
            layers.iter().any(|l| wanted(l) || inner(l))
        }
        any(&self.layers, &wanted)
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        fn find<'a>(layers: &'a [Layer], id: &str) -> Option<&'a Layer> {
            layers.iter().find_map(|l| if l.id == id { Some(l) } else { find(l.children(), id) })
        }
        find(&self.layers, id)
    }

    pub fn paths(&self) -> BTreeSet<&str> {
        let assets = self.assets.values().map(|a| a.path.as_str());
        let effects = self.effects.values().flat_map(|f| [f.descriptor.as_str(), f.wgsl.as_str()]);
        assets.chain(self.scripts.iter().map(|s| s.path.as_str())).chain(effects).collect()
    }
}
