use crate::action::KeyframeEdit;
use crate::ops::{Op, set};
use anyhow::{Result, ensure};
use serde_json::{Map, Value, json};
use wprs_core::message::said;

fn handle(key: &Value, side: &str) -> Option<[f64; 2]> {
    let pair = key.get(2)?.get(side)?.as_array()?;
    Some([pair.first()?.as_f64()?, pair.get(1)?.as_f64()?])
}

pub(crate) fn curve(a: &Value, b: &Value, t: f64) -> Option<Value> {
    let (out, back) = (handle(a, "out"), handle(b, "in"));
    let ([t0, v0], [t1, v1]) = ([a[0].as_f64()?, a[1].as_f64()?], [b[0].as_f64()?, b[1].as_f64()?]);
    if out.is_none() && back.is_none() {
        return None;
    }
    let third = [(t1 - t0) / 3.0, (v1 - v0) / 3.0];
    let ([dt0, dv0], [dt1, dv1]) = (out.unwrap_or(third), back.unwrap_or(third));
    let at = |p: [f64; 4], s: f64| {
        let r = 1.0 - s;
        r * r * r * p[0] + 3.0 * r * r * s * p[1] + 3.0 * r * s * s * p[2] + s * s * s * p[3]
    };
    let xs = [t0, t0 + dt0, t1 - dt1, t1];
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..32 {
        let mid = (lo + hi) / 2.0;
        (lo, hi) = if at(xs, mid) < t { (mid, hi) } else { (lo, mid) };
    }
    let v = at([v0, v0 + dv0, v1 - dv1, v1], (lo + hi) / 2.0);
    Some(json!((v * 1e6).round() / 1e6))
}

pub(crate) fn shaped(e: &KeyframeEdit) -> Result<()> {
    let handles = [e.in_handle, e.out_handle];
    let fine = |[dt, dv]: [f64; 2]| dt.is_finite() && dv.is_finite() && (0.0..=3600.0).contains(&dt);
    ensure!(
        handles.iter().flatten().all(|h| fine(*h)),
        said(
            "editor.handle_reach",
            "a Bézier handle reaches 0 to 3600 s along the timeline and any finite value",
            &[]
        )
    );
    let handled = handles.iter().any(Option::is_some);
    ensure!(
        !(handled && e.ease.is_some()),
        said("editor.ease_or_handles", "a keyframe eases or has Bézier handles, not both", &[])
    );
    ensure!(
        !(e.base.is_some() && e.absolute),
        said("editor.base_or_absolute", "give a base or make the value absolute, not both", &[])
    );
    Ok(())
}

pub(crate) fn end(old: Option<&Value>, e: &KeyframeEdit, value: &Value) -> Result<Option<Value>> {
    if e.ease.is_some() {
        return Ok(e.ease.clone());
    }
    if e.in_handle.is_none() && e.out_handle.is_none() {
        return Ok(old.cloned());
    }
    ensure!(
        value.is_number(),
        said("editor.handles_numbers", "Bézier handles shape keyframes of single numbers", &[])
    );
    let mut sides = old.and_then(Value::as_object).cloned().unwrap_or_else(Map::new);
    let round = |[dt, dv]: [f64; 2]| json!([(dt * 1000.0).round() / 1000.0, (dv * 1e6).round() / 1e6]);
    for (side, h) in [("in", e.in_handle), ("out", e.out_handle)] {
        if let Some(h) = h {
            sides.insert(side.into(), round(h));
        }
    }
    Ok(Some(Value::Object(sides)))
}

pub(crate) fn key(time: f64, value: Value, third: Option<Value>) -> Value {
    third.map_or_else(|| json!([time, value]), |t| json!([time, value, t]))
}

fn kind(v: &Value) -> Option<usize> {
    match v {
        Value::Number(_) => Some(1),
        Value::Array(list) if list.iter().all(Value::is_number) => Some(10 + list.len()),
        Value::String(text) => wprs_scene::Color::parse(text).map(|_| 0),
        _ => None,
    }
}

pub(crate) fn based(staged: &Value, e: &KeyframeEdit) -> Result<Vec<Op>> {
    let pointer = e.pointer.as_str();
    let at = format!("{pointer}/base");
    if e.absolute {
        return Ok(staged.pointer(&at).map(|_| Op::Remove { path: at.clone() }).into_iter().collect());
    }
    let Some(base) = &e.base else { return Ok(Vec::new()) };
    let first = staged.pointer(&format!("{pointer}/keys/0/1"));
    ensure!(
        !e.remove && first.is_some(),
        said("editor.base_keys", "only a value with keyframes adds them to a base", &[])
    );
    let fits = kind(base).is_some() && kind(base) == first.and_then(kind);
    ensure!(
        fits,
        said(
            "editor.base_kind",
            "the base of %1 is a number, a list of numbers or a color like its keyframes",
            &[&pointer]
        )
    );
    Ok(set(staged, &at, base.clone()))
}
