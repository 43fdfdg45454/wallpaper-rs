use serde_json::Value;
use std::path::Path;
use wprs_editor::ops::Op;
use wprs_editor::{Action, Project, assets, packing, placing, previews};

pub enum Placed {
    Asset(String, (bool, Option<[u32; 2]>)),
    Preview(bool),
    Packed(String),
    Layer(String, Box<Action>),
}

pub struct Landed {
    placed: Placed,
    path: String,
    doc: Value,
    ops: Option<Vec<Op>>,
    fresh: bool,
}

fn ops(dir: &Path, doc: &Value, placed: &Placed, path: &str) -> anyhow::Result<Vec<Op>> {
    match placed {
        Placed::Asset(id, cut) => assets::sheet(dir, doc, id, path, *cut),
        Placed::Preview(animated) => previews::adopt(doc, *animated, path),
        Placed::Packed(id) => Ok(packing::adopt(doc, id, path)),
        Placed::Layer(asset, add) => placing::placed(dir, doc, add, asset, path),
    }
}

pub fn land(dir: &Path, doc: Value, action: Action) -> anyhow::Result<Landed> {
    let (placed, path, fresh) = match action {
        Action::Thumbnail { file, animated } => {
            (Placed::Preview(animated), previews::store(dir, &doc, &file, animated)?, false)
        }
        Action::Import { file, id, linear, atlas } => {
            assets::cuttable(&file.to_string_lossy(), (linear, atlas))?;
            let (id, path, fresh) = assets::imported(dir, &doc, &file, id.as_deref())?;
            (Placed::Asset(id, (linear, atlas)), path, fresh)
        }
        Action::Asset(wanted) => {
            (Placed::Packed(wanted.asset.clone()), packing::store(dir, &doc, &wanted)?, false)
        }
        placing @ Action::AddLayer { file: Some(_), .. } => {
            let (asset, path, fresh) = placing::import(dir, &doc, &placing)?;
            (Placed::Layer(asset, Box::new(placing)), path, fresh)
        }
        other => return Err(anyhow::anyhow!("{} is not a file action", other.name())),
    };
    let ops = match &placed {
        Placed::Asset(..) | Placed::Layer(..) => {
            Some(assets::undone(dir, (&path, fresh), ops(dir, &doc, &placed, &path))?)
        }
        Placed::Preview(_) | Placed::Packed(_) => None,
    };
    Ok(Landed { placed, path, doc, ops, fresh })
}

pub fn settle(project: &mut Project, landed: Landed, label: &str) -> anyhow::Result<Vec<Op>> {
    let Landed { placed, path, doc, ops: ready, fresh } = landed;
    let same = |key: &str| project.doc[key] == doc[key];
    let ops = match ready.filter(|_| same("scene") && same("manifest")) {
        Some(ready) => Ok(ready),
        None => ops(&project.dir, &project.doc, &placed, &path),
    };
    let dir = project.dir.clone();
    assets::undone(&dir, (&path, fresh), ops.and_then(|ops| project.record(label, ops)))
}
