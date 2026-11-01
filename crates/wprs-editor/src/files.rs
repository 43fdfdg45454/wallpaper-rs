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
        action => return crate::rigging::act(dir, doc, action),
    })
}
