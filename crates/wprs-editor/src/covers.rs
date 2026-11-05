use crate::action::AssetEdit;
use crate::layers::asset_kind;
use crate::ops::{Op, diff, escape};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;

pub const KINDS: [&str; 3] = ["current", "previous", "none"];

fn picture(doc: &Value, wanted: &str) -> Result<String> {
    let asset = doc.pointer(&format!("/scene/assets/{}/path", escape(wanted))).and_then(Value::as_str);
    let path = asset.unwrap_or(wanted).to_owned();
    let still = asset_kind(&path) == "image" && !path.ends_with(".gif");
    ensure!(
        still && wprs_format::check_path(&path).is_ok(),
        said("editor.cover_placeholder", "%1 is not a still picture of the scene", &[&wanted])
    );
    Ok(path)
}

pub(crate) fn act(doc: &Value, wanted: &AssetEdit) -> Result<Vec<Op>> {
    let id = &wanted.asset;
    let pointer = format!("/scene/assets/{}", escape(id));
    let old = doc.pointer(&pointer).cloned();
    let mut asset = old.clone().unwrap_or_else(|| json!({}));
    if let Some(p) = &wanted.placeholder {
        asset["path"] = json!(picture(doc, p)?);
    }
    ensure!(
        asset["path"].is_string(),
        said("editor.cover_new", "a new cover %1 needs a placeholder picture", &[id])
    );
    match wanted.cover.as_deref() {
        None => {}
        Some("none") => drop(asset.as_object_mut().map(|a| a.remove("media"))),
        Some(kind @ ("current" | "previous")) => asset["media"] = json!(kind),
        Some(other) => {
            return Err(anyhow!(said("editor.cover_kind", "%1 is not current, previous or none", &[&other])));
        }
    }
    if asset.get("media").is_some() {
        let a = asset.as_object_mut().into_iter().flatten();
        let moving = ["frames", "fps", "durations", "rects", "atlas", "animated"];
        ensure!(
            !a.into_iter().any(|(k, _)| moving.contains(&k.as_str())),
            said("editor.cover_still", "%1 is animated, and a cover is a still picture", &[id])
        );
    }
    let mut new = doc.clone();
    new["scene"]["assets"][id.as_str()] = asset;
    Ok(diff(doc, &new, ""))
}
