use super::{field, figure};
use crate::action::PlayEdit;
use crate::clips::num;
use crate::ops::Op;
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::MAX_ANIMATION_LAYERS;

fn bounded(name: &str, v: Option<f64>, lo: f64, hi: f64) -> Result<Option<Value>> {
    let Some(v) = v else { return Ok(None) };
    ensure!(
        v.is_finite() && (lo..=hi).contains(&v),
        said("editor.play_range", "%1 of an animation layer goes from %2 to %3", &[&name, &lo, &hi])
    );
    Ok(Some(num(v as f32)))
}

fn clip_known(l: &Value, clip: &str) -> Result<()> {
    crate::clips::found(l, clip).map(|_| ())
}

pub(super) fn play(dir: &Path, doc: &Value, e: &PlayEdit) -> Result<Vec<Op>> {
    let (path, l) = figure(doc, &e.layer)?;
    let mut plays = l["animations"].as_array().cloned().unwrap_or_default();
    let i = match &e.add {
        Some(clip) => {
            clip_known(l, clip)?;
            ensure!(
                plays.len() < MAX_ANIMATION_LAYERS,
                said(
                    "editor.plays_most",
                    "a layer has at most %1 animation layers",
                    &[&MAX_ANIMATION_LAYERS]
                )
            );
            plays.push(json!({ "clip": clip }));
            plays.len() - 1
        }
        None => e.index.filter(|i| *i < plays.len()).ok_or_else(|| {
            anyhow!(said("editor.no_play", "there is no animation layer %1", &[&e.index.unwrap_or(0)]))
        })?,
    };
    if e.remove {
        plays.remove(i);
        return Ok(field(doc, &path, "animations", (!plays.is_empty()).then(|| json!(plays))));
    }
    let a = &mut plays[i];
    if let Some(c) = &e.clip {
        clip_known(l, c)?;
        a["clip"] = json!(c);
    }
    let numbers = [
        ("weight", bounded("weight", e.weight, 0.0, 1.0)?),
        ("rate", bounded("rate", e.rate, 0.0, 16.0)?),
        ("blend_in", bounded("blend_in", e.blend_in, 0.0, 10.0)?),
        ("blend_out", bounded("blend_out", e.blend_out, 0.0, 10.0)?),
    ];
    for (k, v) in numbers.into_iter().filter_map(|(k, v)| Some((k, v?))) {
        a[k] = v;
    }
    let flags = [("additive", e.additive, false), ("enabled", e.enabled, true)];
    for (k, v, default) in flags.into_iter().filter_map(|(k, v, d)| Some((k, v?, d))) {
        match v == default {
            true => drop(a.as_object_mut().and_then(|m| m.remove(k))),
            false => a[k] = json!(v),
        }
    }
    if let Some(bones) = &e.bones {
        let (_, _, rig) = crate::skin::rigged(dir, doc, l)?;
        if let Some(b) = bones.iter().find(|b| rig.joint(b).is_none()) {
            return Err(anyhow!(said("editor.no_bone", "there is no bone \"%1\"", &[b])));
        }
        match bones.is_empty() {
            true => drop(a.as_object_mut().and_then(|m| m.remove("bones"))),
            false => a["bones"] = json!(bones),
        }
    }
    if let Some(to) = e.to {
        let moved = plays.remove(i);
        plays.insert(to.min(plays.len()), moved);
    }
    Ok(field(doc, &path, "animations", Some(json!(plays))))
}
