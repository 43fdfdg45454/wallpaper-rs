use crate::action::{DepthSet, PartsEdit};
use crate::clips::num;
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Map, Value, json};
use std::path::Path;
use wprs_core::message::said;

fn clipped(map: &Map<String, Value>, from: &str, to: &str) -> bool {
    let mut at = to.to_owned();
    for _ in 0..=map.len() {
        if at == from {
            return true;
        }
        match map.get(&at).and_then(|p| p["clip_to"].as_str()) {
            Some(next) => at = next.to_owned(),
            None => return false,
        }
    }
    true
}

fn depth(doc: &Value, path: &str, layer: &Value, d: &DepthSet) -> Result<Vec<Op>> {
    let at = format!("{path}/depth");
    let given = d.scale.is_some() || d.cull.is_some() || d.sort.is_some();
    ensure!(!(d.off && given), said("editor.depth_off", "depth is turned off or changed, not both", &[]));
    if d.off {
        return Ok(layer.get("depth").map(|_| Op::Remove { path: at }).into_iter().collect());
    }
    let mut v = layer["depth"].as_object().cloned().unwrap_or_default();
    if let Some(s) = d.scale {
        let bad = said("editor.depth_scale", "the depth scale goes from 0 to 10", &[]);
        ensure!(s.is_finite() && (0.0..=10.0).contains(&s), bad);
        v.insert("scale".into(), num(s));
    }
    if let Some(c) = d.cull {
        v.insert("cull".into(), json!(c));
    }
    match d.sort {
        Some(false) => drop(v.insert("sort".into(), json!(false))),
        Some(true) => drop(v.remove("sort")),
        None => {}
    }
    Ok(set(doc, &at, Value::Object(v)))
}

pub(crate) fn parts(dir: &Path, doc: &Value, p: &PartsEdit) -> Result<Vec<Op>> {
    let (path, layer) = crate::actions::layer(doc, &p.layer)?;
    let (_, _, rig) = crate::skin::rigged(dir, doc, layer)?;
    let deep = p.depth.as_ref().map(|d| depth(doc, &path, layer, d)).transpose()?.unwrap_or_default();
    let clips = !p.clip.is_empty() || !p.unclip.is_empty();
    if p.order.is_empty() && p.hide.is_empty() && p.show.is_empty() && !clips && p.depth.is_some() {
        return Ok(deep);
    }
    let known = |n: &str| {
        let no = || anyhow!(said("editor.no_part", "there is no part \"%1\"", &[&n]));
        rig.parts.iter().any(|x| x == n).then_some(()).ok_or_else(no)
    };
    let given = !p.order.is_empty() || !p.hide.is_empty() || !p.show.is_empty();
    ensure!(
        given || !p.clip.is_empty() || !p.unclip.is_empty(),
        said("editor.parts_what", "say what changes in the parts", &[])
    );
    ensure!(
        !p.hide.iter().any(|h| p.show.contains(h)),
        said("editor.part_shown", "a part is hidden or shown, not both", &[])
    );
    let mut map = layer["parts"].as_object().cloned().unwrap_or_default();
    let slot = |map: &mut Map<String, Value>, n: &str, key: &str, v: Option<Value>| -> Result<()> {
        known(n)?;
        let part = map.entry(n.to_owned()).or_insert_with(|| json!({}));
        if let Some(o) = part.as_object_mut() {
            match v {
                Some(v) => drop(o.insert(key.into(), v)),
                None => drop(o.remove(key)),
            }
        }
        Ok(())
    };
    for (n, order) in &p.order {
        ensure!(
            order.is_finite() && (-1000.0..=1000.0).contains(order),
            said("editor.part_order", "the order of a part goes from -1000 to 1000", &[])
        );
        slot(&mut map, n, "order", Some(num(order.round())))?;
    }
    p.hide.iter().try_for_each(|n| slot(&mut map, n, "visible", Some(json!(false))))?;
    p.show.iter().try_for_each(|n| slot(&mut map, n, "visible", None))?;
    p.unclip.iter().try_for_each(|n| slot(&mut map, n, "clip_to", None))?;
    for (n, to) in &p.clip {
        known(to)?;
        ensure!(
            !clipped(&map, n, to),
            said("editor.part_clip", "a part shows over another part, not over itself", &[])
        );
        slot(&mut map, n, "clip_to", Some(json!(to)))?;
    }
    map.retain(|_, v| v.as_object().is_none_or(|o| !o.is_empty()));
    let at = format!("{path}/{}", escape("parts"));
    let mut ops = match (map.is_empty(), layer.get("parts").is_some()) {
        (true, true) => vec![Op::Remove { path: at }],
        (true, false) => Vec::new(),
        (false, _) => set(doc, &at, Value::Object(map)),
    };
    ops.extend(deep);
    Ok(ops)
}
