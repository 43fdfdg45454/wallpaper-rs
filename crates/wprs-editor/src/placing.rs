use crate::actions::{Action, plan};
use crate::layers::asset_kind;
use crate::ops::{Op, apply};
use anyhow::{Result, anyhow, ensure};
use serde_json::Value;
use std::path::Path;
use wprs_core::message::said;

const SHOWN: [(&str, &str); 5] =
    [("image", "image"), ("video", "video"), ("sound", "sound"), ("mesh", "model"), ("model", "model")];

pub fn fits(kind: &str, file: &Path) -> Result<()> {
    let wanted = SHOWN.iter().find(|(k, _)| *k == kind).map(|(_, f)| *f);
    let found = asset_kind(&file.to_string_lossy());
    ensure!(
        wanted.is_some_and(|w| w == found),
        said(
            "editor.layer_file",
            "a %1 layer cannot show %2; image, video, sound, mesh and model layers take a file of their kind",
            &[&kind, &file.display()]
        )
    );
    Ok(())
}

pub fn placed(dir: &Path, doc: &Value, action: &Action, asset: &str, path: &str) -> Result<Vec<Op>> {
    let Action::AddLayer { kind, id, name, into, at, .. } = action else { return Ok(Vec::new()) };
    let mut ops = crate::assets::adopt(doc, asset, path)?;
    let mut staged = doc.clone();
    apply(&mut staged, &ops)?;
    let add = Action::AddLayer {
        kind: kind.clone(),
        id: id.clone(),
        name: name.clone(),
        source: Some(asset.into()),
        into: into.clone(),
        at: *at,
        file: None,
    };
    let layer = plan(&staged, &add)?;
    if let Some(Op::Add { value, .. }) = layer.last() {
        match kind.as_str() {
            "mesh" => drop(crate::skin::rigged(dir, &staged, value)?),
            "model" => drop(crate::stage3d::file(&dir.join(path))?),
            _ => {}
        }
    }
    ops.extend(layer);
    Ok(ops)
}

pub fn import(dir: &Path, doc: &Value, action: &Action) -> Result<(String, String)> {
    let Action::AddLayer { kind, file: Some(file), id, .. } = action else {
        return Err(anyhow!(said("editor.name_a_file", "name a file", &[])));
    };
    fits(kind, file)?;
    crate::assets::import(dir, doc, file, named(doc, file, id.as_deref()).as_deref())
}

pub(crate) fn act(dir: &Path, doc: &Value, action: &Action) -> Option<Result<Vec<Op>>> {
    let Action::AddLayer { file: Some(_), .. } = action else { return None };
    Some(import(dir, doc, action).and_then(|(asset, path)| placed(dir, doc, action, &asset, &path)))
}

fn named(doc: &Value, file: &Path, layer: Option<&str>) -> Option<String> {
    let stem = file.file_stem()?.to_string_lossy().into_owned();
    let layer = layer?;
    (crate::layers::unique(doc, &stem) == layer).then(|| crate::layers::unique(doc, &format!("{layer}_file")))
}
