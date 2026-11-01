use crate::{
    EffectControl, LayerOverride, ParticleControl, ScriptOutputs, SoundControl, TimelineCommand, VideoControl,
};
use serde::{Deserialize, Deserializer};
use serde_json::Value as Json;
use std::collections::BTreeMap;

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

macro_rules! patch {
    ($($field:ident: $kind:ty),*) => {
        #[derive(Deserialize)]
        struct Patch {
            $(#[serde(default, deserialize_with = "present")] $field: Option<$kind>,)*
            #[serde(default)]
            layer_edits: BTreeMap<String, Option<LayerOverride>>,
        }

        impl ScriptOutputs {
            pub fn patch(&mut self, text: &str) -> Result<(), serde_json::Error> {
                let patch: Patch = serde_json::from_str(text)?;
                $(if let Some(v) = patch.$field { self.$field = v; })*
                for (id, edit) in patch.layer_edits {
                    match edit {
                        Some(o) => self.layers.insert(id, o),
                        None => self.layers.remove(&id),
                    };
                }
                Ok(())
            }
        }
    };
}

patch!(
    values: BTreeMap<String, Json>,
    layers: BTreeMap<String, LayerOverride>,
    timelines: Vec<(String, TimelineCommand)>,
    particles: BTreeMap<String, ParticleControl>,
    sounds: BTreeMap<String, SoundControl>,
    videos: BTreeMap<String, VideoControl>,
    effects: BTreeMap<String, Vec<EffectControl>>,
    spawned: Vec<(String, String)>,
    order: Option<Vec<String>>,
    watch: Vec<String>,
    wake: Option<f64>,
    busy: bool,
    console: Vec<String>,
    stopped: Vec<String>
);
