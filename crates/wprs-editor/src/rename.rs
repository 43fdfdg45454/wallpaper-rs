use crate::ops::{Op, set};
use crate::tidy::Tree;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(crate) enum Change<'a> {
    Renamed(&'a str, &'a str),
    Gone(&'a str, Option<&'a str>),
    Moved,
}

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

fn follower(l: &mut Value, change: Change) {
    let Some(a) = l.get_mut("attach") else { return };
    match change {
        Change::Renamed(old, new) => {
            swap(a.get_mut("bone"), old, new);
            swap(a.get_mut("point"), old, new);
        }
        Change::Gone(gone, _) if a["bone"] == gone || a["point"] == gone => {
            drop(l.as_object_mut().and_then(|m| m.remove("attach")))
        }
        _ => {}
    }
}

pub(crate) fn bone(doc: &Value, path: &str, change: Change, trees: (&Tree, &Tree)) -> Vec<Op> {
    let Some(id) = doc.pointer(path).map(|l| l["id"].clone()) else { return Vec::new() };
    let mut ops = Vec::new();
    for (at, before) in crate::layers::walk(doc) {
        let mut after = before.clone();
        if at == path {
            let (gone, keep) = match change {
                Change::Renamed(old, new) => {
                    layer(&mut after, old, new);
                    (None, None)
                }
                Change::Gone(g, keep) => (Some(g), keep),
                Change::Moved => (None, None),
            };
            crate::tidy::layer(&mut after, trees.0, trees.1, gone, keep);
        }
        if before["attach"]["layer"] == id {
            follower(&mut after, change);
        }
        let (Some(a), Some(b)) = (before.as_object(), after.as_object()) else { continue };
        let fields = b.iter().filter(|(k, v)| *k != "layers" && a.get(*k) != Some(*v));
        ops.extend(fields.flat_map(|(k, v)| set(doc, &format!("{at}/{k}"), v.clone())));
        let removed = a.keys().filter(|k| !b.contains_key(*k));
        ops.extend(removed.map(|k| Op::Remove { path: format!("{at}/{k}") }));
    }
    ops
}
