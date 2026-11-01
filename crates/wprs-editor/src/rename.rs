use crate::ops::{Op, set};
use serde_json::{Value, json};

fn swap(v: Option<&mut Value>, old: &str, new: &str) {
    if let Some(v) = v.filter(|v| *v == old) {
        *v = json!(new);
    }
}

fn key(map: Option<&mut Value>, old: &str, new: &str) {
    if let Some(m) = map.and_then(Value::as_object_mut)
        && let Some(v) = m.remove(old)
    {
        m.insert(new.to_owned(), v);
    }
}

fn each<'a>(v: &'a mut Value, field: &str) -> impl Iterator<Item = &'a mut Value> {
    v.get_mut(field).and_then(Value::as_array_mut).into_iter().flatten()
}

fn layer(l: &mut Value, old: &str, new: &str) {
    each(l, "clips").for_each(|c| key(c.get_mut("bones"), old, new));
    each(l, "animations").for_each(|a| each(a, "bones").for_each(|b| swap(Some(b), old, new)));
    for chain in each(l, "ik") {
        each(chain, "chain").for_each(|b| swap(Some(b), old, new));
        key(chain.get_mut("limits"), old, new);
        swap(chain.get_mut("target"), &format!("bone:{old}"), &format!("bone:{new}"));
    }
    each(l, "physics").for_each(|p| swap(p.get_mut("bone"), old, new));
    for r in each(l, "rules") {
        swap(r.get_mut("bone"), old, new);
        swap(r.get_mut("parent"), old, new);
    }
    each(l, "hit").for_each(|b| swap(Some(b), old, new));
}

pub(crate) fn bone(doc: &Value, path: &str, old: &str, new: &str) -> Vec<Op> {
    let Some(id) = doc.pointer(path).map(|l| l["id"].clone()) else { return Vec::new() };
    let mut ops = Vec::new();
    for (at, before) in crate::layers::walk(doc) {
        let mut after = before.clone();
        if at == path {
            layer(&mut after, old, new);
        }
        if before["attach"]["layer"] == id
            && let Some(a) = after.get_mut("attach")
        {
            swap(a.get_mut("bone"), old, new);
            swap(a.get_mut("point"), old, new);
        }
        let (Some(a), Some(b)) = (before.as_object(), after.as_object()) else { continue };
        let fields = b.iter().filter(|(k, v)| *k != "layers" && a.get(*k) != Some(*v));
        ops.extend(fields.flat_map(|(k, v)| set(doc, &format!("{at}/{k}"), v.clone())));
    }
    ops
}
