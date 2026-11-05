use super::tend::listed;
use super::{parsed, render, system};
use crate::action::SowEdit as S;
use crate::ops::{Op, escape, set};
use anyhow::{Result, bail, ensure};
use serde_json::{Map, Value, json};
use wprs_core::message::said;
use wprs_scene::{Child, Frames, Moment};

const RANGES: [&str; 14] = [
    "life", "speed", "angle", "size", "rotation", "spin", "frame", "color", "alpha", "velocity", "radial",
    "hsv", "palette", "gradient",
];
const TINTS: [&str; 3] = ["hsv", "palette", "gradient"];
const CLEARED: [&str; 6] = ["alpha", "velocity", "radial", "hsv", "palette", "gradient"];

fn object(doc: &Value, at: &str) -> Map<String, Value> {
    doc.pointer(at).and_then(Value::as_object).cloned().unwrap_or_default()
}

fn create(
    doc: &Value,
    id: &str,
    from: Option<&str>,
    parent: Option<&str>,
    on: Option<Moment>,
) -> Result<Vec<Op>> {
    let path = format!("/scene/particles/{}", escape(id));
    ensure!(doc.pointer(&path).is_none(), said("editor.id_use", "the id %1 is in use", &[&id]));
    let value = match from {
        Some(f) => doc.pointer(&system(doc, f)?).cloned().unwrap_or_default(),
        None => crate::layers::system(),
    };
    let mut ops = set(doc, &path, value);
    if let Some(p) = parent {
        let at = format!("{}/children", system(doc, p)?);
        let mut list = doc.pointer(&at).and_then(Value::as_array).cloned().unwrap_or_default();
        list.push(json!({ "system": id, "on": on.unwrap_or(Moment::Static) }));
        ops.extend(set(doc, &at, json!(list)));
    }
    Ok(ops)
}

fn rate(doc: &Value, sys: &str, rate: f64, (emitter, replace): (Option<usize>, bool)) -> Result<Vec<Op>> {
    let count = doc.pointer(&format!("{sys}/emitters")).and_then(Value::as_array).map_or(0, Vec::len);
    let missing = emitter.filter(|i| *i >= count).or((count == 0).then_some(0));
    ensure!(
        missing.is_none(),
        said("editor.no_emitter", "the system has no emitter %1", &[&missing.unwrap_or(0)])
    );
    let mut ops = Vec::new();
    for i in emitter.map_or(0..count, |i| i..i + 1) {
        let at = format!("{sys}/emitters/{i}/rate");
        crate::shaping::plain(doc, &at, replace)?;
        ops.extend(set(doc, &at, json!(rate)));
    }
    Ok(ops)
}

fn start(doc: &Value, sys: &str, ranges: &Map<String, Value>, clear: &[String]) -> Result<Vec<Op>> {
    let at = format!("{sys}/init");
    let mut init = object(doc, &at);
    for key in ranges.keys() {
        let known = RANGES.contains(&key.as_str());
        ensure!(
            known,
            said("editor.no_start_range", "%1 is not a starting range: %2", &[key, &RANGES.join(", ")])
        );
    }
    for key in clear {
        let known = CLEARED.contains(&key.as_str());
        ensure!(
            known,
            said("editor.start_cleared", "only %1 can be cleared, not %2", &[&CLEARED.join(", "), key])
        );
        init.remove(key);
    }
    let colors = ranges.get("color").and_then(Value::as_array).into_iter().flatten();
    if let Some(bad) = colors.filter_map(Value::as_str).find(|c| wprs_scene::Color::parse(c).is_none()) {
        bail!(said("editor.not_color", "%1 is not a color such as #ffcc88", &[&bad]));
    }
    let tints = TINTS.iter().filter(|t| ranges.contains_key(**t)).count();
    ensure!(
        tints <= 1,
        said("editor.one_tint", "particles start with hues, a palette or a gradient, one of them", &[])
    );
    if tints == 1 {
        TINTS.iter().for_each(|t| drop(init.remove(*t)));
    }
    ranges.iter().for_each(|(k, v)| drop(init.insert(k.clone(), v.clone())));
    Ok(set(doc, &at, Value::Object(init)))
}

fn look(doc: &Value, at: &str, change: impl Fn(&mut Map<String, Value>)) -> Vec<Op> {
    let mut render = object(doc, at);
    change(&mut render);
    set(doc, at, Value::Object(render))
}

fn keep(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    match value {
        Some(v) => drop(map.insert(key.into(), v)),
        None => drop(map.remove(key)),
    }
}

pub(super) fn edit(doc: &Value, e: &S) -> Result<Vec<Op>> {
    Ok(match e {
        S::Create { system: id, from, parent, on } => {
            create(doc, id, from.as_deref(), parent.as_deref(), *on)?
        }
        S::Delete { system: s } => vec![Op::Remove { path: system(doc, s)? }],
        S::Settings { system: s, capacity, seed, prewarm, space, rate: r, emitter, replace, lock } => {
            let sys = system(doc, s)?;
            let mut ops = Vec::new();
            capacity.iter().for_each(|c| ops.extend(set(doc, &format!("{sys}/capacity"), json!(c))));
            seed.iter().for_each(|c| ops.extend(set(doc, &format!("{sys}/seed"), json!(c))));
            prewarm.iter().for_each(|c| ops.extend(set(doc, &format!("{sys}/prewarm"), json!(c))));
            space.iter().for_each(|c| ops.extend(set(doc, &format!("{sys}/space"), json!(c))));
            if let Some(lock) = lock {
                ops.extend(super::emits::lock(doc, &sys, lock)?);
            }
            if let Some(r) = r {
                ops.extend(rate(doc, &sys, *r, (*emitter, *replace))?);
            }
            ops
        }
        S::Start { system: s, ranges, clear } => start(doc, &system(doc, s)?, ranges, clear)?,
        S::Child { system: s, index, child, remove } => {
            let at = format!("{}/children", system(doc, s)?);
            let missing = |i: usize| said("editor.no_child", "the system has no child %1", &[&i]);
            listed::<Child>(doc, &at, (*index, child.as_ref(), *remove), ("child", missing))?
        }
        S::Over { system: s, layer, size, color } => {
            render(doc, &system(doc, s)?, layer.as_deref(), |d, at| {
                look(d, at, |r| {
                    let filled = |v: Value| v.as_array().is_some_and(|l| !l.is_empty()).then_some(v);
                    size.iter().for_each(|k| keep(r, "size_over_life", filled(json!(k))));
                    color.iter().for_each(|c| keep(r, "color_over_life", filled(json!(c))));
                })
            })?
        }
        S::Render { system: s, layer, stretch, frames, fps, mix, brightness, cycles, order } => {
            if let Some(o) = order.as_deref().filter(|o| *o != "none") {
                parsed::<wprs_scene::Stack>(&json!(o), "order")?;
            }
            if let Some(f) = frames.as_deref().filter(|f| *f != "none") {
                parsed::<Frames>(&json!(f), "frames")?;
            }
            render(doc, &system(doc, s)?, layer.as_deref(), |d, at| {
                look(d, at, |r| {
                    let still = |v: &Value| v.as_f64() == Some(0.0);
                    stretch.iter().for_each(|s| keep(r, "stretch", Some(s.clone()).filter(|s| !still(s))));
                    frames.iter().for_each(|f| keep(r, "frames", (f != "none").then(|| json!(f))));
                    fps.iter().for_each(|f| keep(r, "fps", Some(json!(f))));
                    mix.iter().for_each(|m| keep(r, "mix", m.then_some(json!(true))));
                    brightness.iter().for_each(|b| keep(r, "brightness", Some(json!(b))));
                    cycles.iter().for_each(|c| keep(r, "cycles", (*c > 0).then(|| json!(c))));
                    order.iter().for_each(|o| keep(r, "order", (o != "none").then(|| json!(o))));
                })
            })?
        }
        S::Emit { system: s, emitter, values, clear } => {
            super::emits::emit(doc, &system(doc, s)?, *emitter, values, clear)?
        }
        S::Window { system: s, index, window } => {
            super::emits::window(doc, &system(doc, s)?, *index, *window)?
        }
        S::Rename { system: s, to } => super::emits::rename(doc, &system(doc, s)?, s, to)?,
    })
}
