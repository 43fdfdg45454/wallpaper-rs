use crate::actions::Action;
use crate::ops::Op;
use crate::paint::{Stroke, paint};
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub(crate) fn act(dir: &Path, doc: &Value, action: &Action) -> Option<Result<Vec<Op>>> {
    Some(match action {
        Action::Import { file, id } => crate::assets::import(dir, doc, file, id.as_deref())
            .and_then(|(id, path)| crate::assets::adopt(doc, &id, &path)),
        Action::ImportEffect { descriptor, wgsl } => {
            crate::shaders::import(dir, doc, descriptor, wgsl.as_deref())
        }
        Action::ImportScript { file } => crate::scripts::import(dir, doc, file),
        Action::Thumbnail { file, animated } => crate::previews::store(dir, doc, file, *animated)
            .and_then(|path| crate::previews::adopt(doc, *animated, &path)),
        Action::PaintMask { pointer, channel, radius, hardness, value, points } => {
            let (channel, radius, hardness, value) = (*channel, *radius, *hardness, *value);
            paint(dir, doc, &Stroke { pointer, channel, radius, hardness, value, points })
        }
        Action::MeshFromPicture { image, outline, cells, id } => {
            let (outline, cells, id) = (*outline, *cells, id.as_deref());
            crate::rig::mesh_from_picture(dir, doc, &crate::rig::Cutting { image, outline, cells, id })
        }
        Action::Bone { layer, name, parent, at, remove } => {
            let (parent, at, remove) = (parent.as_deref(), *at, *remove);
            crate::rig::bone(dir, doc, &crate::rig::Boning { layer, name, parent, at, remove })
        }
        _ => return None,
    })
}
