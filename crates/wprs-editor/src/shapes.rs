use crate::action::ExpressionEdit;
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Map, Value};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::MAX_EXPRESSIONS;

pub(crate) fn expression(dir: &Path, doc: &Value, e: &ExpressionEdit) -> Result<Vec<Op>> {
    let ExpressionEdit { layer, name, shapes, remove, to } = e;
    let (name, remove, to) = (name.as_str(), *remove, to.as_deref());
    let (path, l) = crate::actions::layer(doc, layer)?;
    let (_, _, rig) = crate::skin::rigged(dir, doc, l)?;
    let named = |n: &str| (1..=64).contains(&n.chars().count());
    ensure!(
        named(name),
        said("editor.expression_name", "an expression has a name of 1 to 64 characters", &[])
    );
    let mut list = l["expressions"].as_object().cloned().unwrap_or_default();
    let mut weights = l["shapes"].as_object().cloned().unwrap_or_default();
    let no = || anyhow!(said("editor.no_expression", "there is no expression \"%1\"", &[&name]));
    if remove {
        list.remove(name).ok_or_else(no)?;
        weights.remove(name);
    } else if let Some(to) = to {
        ensure!(
            named(to),
            said("editor.expression_name", "an expression has a name of 1 to 64 characters", &[])
        );
        ensure!(
            !list.contains_key(to),
            said("editor.expression_taken", "there is already an expression \"%1\"", &[&to])
        );
        let set = list.remove(name).ok_or_else(no)?;
        list.insert(to.to_owned(), set);
        if let Some(w) = weights.remove(name) {
            weights.insert(to.to_owned(), w);
        }
    } else {
        ensure!(
            list.contains_key(name) || list.len() < MAX_EXPRESSIONS,
            said("editor.expressions_most", "a mesh has at most %1 expressions", &[&MAX_EXPRESSIONS])
        );
        let mut set = Map::new();
        for (s, w) in shapes {
            let no = || anyhow!(said("editor.no_shape", "there is no blend shape \"%1\"", &[s]));
            rig.shape(s).ok_or_else(no)?;
            ensure!(
                w.is_finite() && (-1.0..=1.0).contains(w),
                said("editor.expression_weight", "an expression weighs each blend shape from -1 to 1", &[])
            );
            set.insert(s.clone(), crate::clips::num(*w));
        }
        list.insert(name.to_owned(), Value::Object(set));
    }
    let mut ops = Vec::new();
    for (key, map) in [("expressions", list), ("shapes", weights)] {
        let at = format!("{path}/{}", escape(key));
        match (map.is_empty(), l.get(key).is_some()) {
            (true, true) => ops.push(Op::Remove { path: at }),
            (true, false) => {}
            (false, _) if l.get(key) != Some(&Value::Object(map.clone())) => {
                ops.extend(set(doc, &at, Value::Object(map)))
            }
            _ => {}
        }
    }
    Ok(ops)
}
