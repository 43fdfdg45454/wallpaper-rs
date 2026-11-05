use crate::actions::Action;
use crate::ops::Op;
use crate::paint::{Stroke, paint};
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub(crate) fn act(dir: &Path, doc: &Value, action: &Action) -> Option<Result<Vec<Op>>> {
    Some(match action {
        Action::Import { file, id, linear, atlas } => crate::assets::import(dir, doc, file, id.as_deref())
            .and_then(|(id, path)| crate::assets::sheet(dir, doc, &id, &path, (*linear, *atlas))),
        Action::ImportEffect { descriptor, wgsl } => {
            crate::shaders::import(dir, doc, descriptor, wgsl.as_deref())
        }
        Action::ImportScript(s) => crate::scripts::import(dir, doc, &s.file, s.template.as_deref()),
        Action::ImportAssets(wanted) => crate::bring::import(dir, doc, wanted),
        Action::Asset(wanted) => crate::packing::act(dir, doc, wanted),
        Action::Rgb(e) => crate::rgb::act(doc, e),
        Action::Thumbnail { file, animated } => crate::previews::store(dir, doc, file, *animated)
            .and_then(|path| crate::previews::adopt(doc, *animated, &path)),
        Action::PaintMask(m) => {
            let (channel, radius, hardness, value) = (m.channel, m.radius, m.hardness, m.value);
            paint(
                dir,
                doc,
                &Stroke { pointer: &m.pointer, channel, radius, hardness, value, points: &m.points },
            )
        }
        action => return crate::rigging::act(dir, doc, action),
    })
}
