use anyhow::{Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum Op {
    Add { path: String, value: Value },
    Remove { path: String },
    Replace { path: String, value: Value },
    Move { from: String, path: String },
}

impl Op {
    pub fn paths(&self) -> Vec<&str> {
        match self {
            Op::Add { path, .. } | Op::Remove { path } | Op::Replace { path, .. } => vec![path],
            Op::Move { from, path } => vec![from, path],
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Op::Add { value, .. } | Op::Replace { value, .. } => value.to_string().len(),
            _ => 0,
        }
    }
}

pub fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn split(path: &str) -> Result<(&str, String)> {
    let (parent, key) = path.rsplit_once('/').ok_or_else(|| anyhow!("{path:?} is not a JSON Pointer"))?;
    ensure!(path.starts_with('/'), "{path:?} is not a JSON Pointer");
    Ok((parent, key.replace("~1", "/").replace("~0", "~")))
}

fn index(key: &str, len: usize) -> Result<usize> {
    let canonical = key == "0" || !key.starts_with('0');
    let i = key.parse::<usize>().ok().filter(|i| canonical && *i <= len);
    i.ok_or_else(|| anyhow!("{key:?} is not an index from 0 to {len}"))
}

fn container<'a>(doc: &'a mut Value, parent: &str) -> Result<&'a mut Value> {
    doc.pointer_mut(parent).ok_or_else(|| anyhow!("{parent} does not exist"))
}

fn add(doc: &mut Value, path: &str, value: Value) -> Result<Op> {
    let (parent, key) = split(path)?;
    match container(doc, parent)? {
        Value::Array(items) => {
            let i = if key == "-" { items.len() } else { index(&key, items.len())? };
            items.insert(i, value);
            Ok(Op::Remove { path: format!("{parent}/{i}") })
        }
        Value::Object(map) => Ok(match map.insert(key, value) {
            Some(old) => Op::Replace { path: path.into(), value: old },
            None => Op::Remove { path: path.into() },
        }),
        _ => bail!("{parent} holds neither a list nor an object"),
    }
}

fn remove(doc: &mut Value, path: &str) -> Result<(String, Value)> {
    let (parent, key) = split(path)?;
    let missing = || anyhow!("{path} does not exist");
    match container(doc, parent)? {
        Value::Array(items) => {
            let i = index(&key, items.len().saturating_sub(1)).ok().filter(|i| *i < items.len());
            let i = i.ok_or_else(missing)?;
            Ok((format!("{parent}/{i}"), items.remove(i)))
        }
        Value::Object(map) => Ok((path.into(), map.remove(&key).ok_or_else(missing)?)),
        _ => Err(missing()),
    }
}

fn apply_one(doc: &mut Value, op: &Op) -> Result<Op> {
    match op {
        Op::Add { path, value } => add(doc, path, value.clone()),
        Op::Remove { path } => remove(doc, path).map(|(path, value)| Op::Add { path, value }),
        Op::Replace { path, value } => {
            let slot = doc.pointer_mut(path).filter(|_| !path.is_empty());
            let slot = slot.ok_or_else(|| anyhow!("{path} does not exist"))?;
            Ok(Op::Replace { path: path.clone(), value: std::mem::replace(slot, value.clone()) })
        }
        Op::Move { from, path } => {
            ensure!(!path.starts_with(&format!("{from}/")), "{from} cannot move into itself");
            let (from, value) = remove(doc, from)?;
            let added = add(doc, path, value.clone());
            if let Ok(Op::Remove { path }) = added {
                return Ok(Op::Move { from: path, path: from });
            }
            if let Ok(Op::Replace { path, value: old }) = &added {
                *container(doc, path)? = old.clone();
            }
            add(doc, &from, value)?;
            Err(added.err().unwrap_or_else(|| anyhow!("{path} already exists")))
        }
    }
}

pub fn apply(doc: &mut Value, ops: &[Op]) -> Result<Vec<Op>> {
    let mut undo = Vec::with_capacity(ops.len());
    for op in ops {
        match apply_one(doc, op) {
            Ok(inverse) => undo.push(inverse),
            Err(e) => {
                undo.iter().rev().for_each(|inverse| drop(apply_one(doc, inverse)));
                return Err(e);
            }
        }
    }
    undo.reverse();
    Ok(undo)
}

pub fn set(doc: &Value, path: &str, value: Value) -> Vec<Op> {
    if doc.pointer(path).is_some() {
        return vec![Op::Replace { path: path.into(), value }];
    }
    let mut ops = Vec::new();
    let mut at = String::new();
    let parts: Vec<&str> = path.split('/').skip(1).collect();
    for part in &parts[..parts.len().saturating_sub(1)] {
        at = format!("{at}/{part}");
        if doc.pointer(&at).is_none() {
            ops.push(Op::Add { path: at.clone(), value: Value::Object(Default::default()) });
        }
    }
    ops.push(Op::Add { path: path.into(), value });
    ops
}

pub fn diff(old: &Value, new: &Value, path: &str) -> Vec<Op> {
    match (old, new) {
        _ if old == new => Vec::new(),
        (Value::Object(a), Value::Object(b)) => {
            let at = |key: &str| format!("{path}/{}", escape(key));
            let gone = a.keys().filter(|k| !b.contains_key(*k)).map(|k| Op::Remove { path: at(k) });
            let mut ops: Vec<Op> = gone.collect();
            for (key, value) in b {
                match a.get(key) {
                    Some(was) => ops.extend(diff(was, value, &at(key))),
                    None => ops.push(Op::Add { path: at(key), value: value.clone() }),
                }
            }
            ops
        }
        _ => vec![Op::Replace { path: path.into(), value: new.clone() }],
    }
}
