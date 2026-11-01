use super::parsed;
use crate::ops::{Op, set};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;

pub(super) fn shape(doc: &Value, sys: &str, emitter: usize, shape: &Value) -> Result<Vec<Op>> {
    parsed::<wprs_scene::EmitShape>(shape, "shape")?;
    let count = doc.pointer(&format!("{sys}/emitters")).and_then(Value::as_array).map_or(0, Vec::len);
    ensure!(emitter <= count, said("editor.no_emitter", "the system has no emitter %1", &[&emitter]));
    if emitter == count {
        let value = json!({ "shape": shape, "rate": 10 });
        return Ok(set(doc, &format!("{sys}/emitters/{emitter}"), value));
    }
    Ok(set(doc, &format!("{sys}/emitters/{emitter}/shape"), shape.clone()))
}

type Spot = (Option<[f64; 2]>, Option<bool>, Option<f64>);

pub(super) fn point(
    doc: &Value,
    sys: &str,
    p: u8,
    (at, follow, angle): Spot,
    remove: bool,
) -> Result<Vec<Op>> {
    ensure!(p < 8, said("editor.control_point", "control points go from p0 to p7", &[]));
    let list = doc.pointer(&format!("{sys}/points")).and_then(Value::as_array).cloned().unwrap_or_default();
    let n = usize::from(p);
    if remove {
        ensure!(n + 1 == list.len(), said("editor.last_point", "only the last control point can go", &[]));
        let path = format!("{sys}/points/{n}");
        return Ok(if n == 0 {
            vec![Op::Remove { path: format!("{sys}/points") }]
        } else {
            vec![Op::Remove { path }]
        });
    }
    let fits = |v: f64| v.is_finite() && v.abs() <= 1e6;
    ensure!(
        at.is_none_or(|[x, y]| fits(x) && fits(y)) && angle.is_none_or(|a| (-360.0..=360.0).contains(&a)),
        said(
            "editor.point_value",
            "a control point lies within a million pixels and turns up to 360 degrees",
            &[]
        )
    );
    let mut list = list;
    while list.len() <= n {
        list.push(json!({ "at": [0, 0] }));
    }
    let mut point = list[n].clone();
    if let Some([x, y]) = at {
        point["at"] = json!([x, y]);
        point.as_object_mut().map(|o| o.remove("follow"));
    }
    match follow {
        Some(true) => {
            point["follow"] = json!("cursor");
            point.as_object_mut().map(|o| o.remove("at"));
        }
        Some(false) if point.get("at").is_none() => {
            point["at"] = json!([0, 0]);
            point.as_object_mut().map(|o| o.remove("follow"));
        }
        _ => {}
    }
    angle.iter().for_each(|a| point["angle"] = json!(a));
    list[n] = point;
    Ok(set(doc, &format!("{sys}/points"), json!(list)))
}
