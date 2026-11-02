use crate::action::AnimEdit;
use crate::ops::{Op, set};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

mod listing;
mod play;
mod presets;

pub use listing::{motion, root_path};
pub use presets::PRESETS;

pub(crate) fn figure<'a>(doc: &'a Value, id: &str) -> Result<(String, &'a Value)> {
    let (path, l) = crate::actions::layer(doc, id)?;
    let ok = l["type"] == "mesh" || l["type"] == "model";
    ensure!(ok, said("editor.not_figure", "the layer is not a mesh or a model", &[]));
    Ok((path, l))
}

pub(crate) fn field(doc: &Value, path: &str, key: &str, value: Option<Value>) -> Vec<Op> {
    let at = format!("{path}/{key}");
    match (value, doc.pointer(&at).is_some()) {
        (None, true) => vec![Op::Remove { path: at }],
        (None, false) => Vec::new(),
        (Some(v), _) => set(doc, &at, v),
    }
}

pub(crate) fn act(dir: &Path, doc: &Value, e: &AnimEdit) -> Result<Vec<Op>> {
    match e {
        AnimEdit::Layer(p) => play::play(dir, doc, p),
        AnimEdit::Root { layer, on } => {
            let (path, l) = figure(doc, layer)?;
            ensure!(
                l["type"] == "model",
                said("editor.not_model_layer", "%1 is not a model layer", &[layer])
            );
            let clips = l["clips"].as_array().is_some_and(|c| !c.is_empty());
            ensure!(
                !on || clips,
                said("editor.root_clips", "root motion needs a clip that moves the root", &[])
            );
            Ok(field(doc, &path, "root_motion", on.then(|| json!(true))))
        }
        AnimEdit::Preset { layer, bone, preset } => presets::preset(dir, doc, (layer, bone), preset),
        AnimEdit::Morph { layer, shape, weight } => {
            let (path, l) = figure(doc, layer)?;
            let (_, _, rig) = crate::skin::rigged(dir, doc, l)?;
            let named = l["expressions"].get(shape).is_some();
            ensure!(
                named || rig.shape(shape).is_some(),
                said("editor.no_shape", "there is no blend shape \"%1\"", &[shape])
            );
            if let Some(w) = weight {
                ensure!(
                    w.is_finite() && (-1.0..=1.0).contains(w),
                    said("editor.shape_weight", "a blend shape weighs from -1 to 1", &[])
                );
            }
            let mut shapes = l["shapes"].as_object().cloned().unwrap_or_default();
            match weight {
                Some(w) => drop(shapes.insert(shape.clone(), crate::clips::num(*w as f32))),
                None => drop(shapes.remove(shape)),
            }
            Ok(field(doc, &path, "shapes", (!shapes.is_empty()).then(|| json!(shapes))))
        }
    }
}
