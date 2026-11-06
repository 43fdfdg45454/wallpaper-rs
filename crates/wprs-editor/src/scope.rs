use crate::ops::{Op, escape};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::sync::Mutex;
use wprs_core::message::said;

const LAYERS: &str = "/scene/layers";
static PICKED: Mutex<Option<String>> = Mutex::new(None);

pub fn pick(template: Option<String>) {
    *PICKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = template;
}

pub(crate) fn picked() -> Option<String> {
    PICKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

pub fn root(name: &str) -> String {
    format!("/scene/templates/{}", escape(name))
}

pub fn of(look: &Value) -> Option<String> {
    look["template"].as_str().filter(|t| !t.is_empty()).map(String::from)
}

pub fn seen<'a>(doc: &'a Value, scope: Option<&str>) -> Result<Cow<'a, Value>> {
    let Some(name) = scope else { return Ok(Cow::Borrowed(doc)) };
    let layer = doc
        .pointer(&root(name))
        .cloned()
        .ok_or_else(|| anyhow!(said("editor.no_template", "there is no template \"%1\"", &[&name])))?;
    let mut seen = doc.clone();
    seen["scene"]["layers"] = json!([layer]);
    Ok(Cow::Owned(seen))
}

fn below(path: &str) -> Option<&str> {
    path.strip_prefix(LAYERS).filter(|rest| rest.is_empty() || rest.starts_with('/'))
}

pub fn shown(path: &str, scope: Option<&str>) -> String {
    let inside =
        below(path).and_then(|r| r.strip_prefix("/0")).filter(|r| r.is_empty() || r.starts_with('/'));
    match (scope, inside) {
        (Some(name), Some(rest)) => format!("{}{rest}", root(name)),
        _ => path.to_owned(),
    }
}

enum Way {
    Add,
    Remove,
    Keep,
}

fn real(path: String, name: &str, way: Way, gone: &mut bool) -> Result<String> {
    let Some(rest) = below(&path) else { return Ok(path) };
    let whole = rest == "/0"
        && match way {
            Way::Add => std::mem::take(gone),
            Way::Remove => {
                *gone = true;
                true
            }
            Way::Keep => true,
        };
    let inside = rest.strip_prefix("/0").filter(|r| r.starts_with('/') || whole);
    let one = || {
        anyhow!(said("editor.template_one", "a template holds one layer; add layers inside its group", &[]))
    };
    inside.map(|r| format!("{}{r}", root(name))).ok_or_else(one)
}

pub fn ops(ops: Vec<Op>, scope: Option<&str>) -> Result<Vec<Op>> {
    let Some(name) = scope else { return Ok(ops) };
    let mut gone = false;
    let mut real = |path: String, way: Way| real(path, name, way, &mut gone);
    let mapped = ops.into_iter().map(|op| {
        Ok(match op {
            Op::Add { path, value } => Op::Add { path: real(path, Way::Add)?, value },
            Op::Remove { path } => Op::Remove { path: real(path, Way::Remove)? },
            Op::Replace { path, value } => Op::Replace { path: real(path, Way::Keep)?, value },
            Op::Move { from, path } => Op::Move { from: real(from, Way::Keep)?, path: real(path, Way::Add)? },
        })
    });
    mapped.collect()
}

impl crate::project::Project {
    pub fn seen(&self) -> Cow<'_, Value> {
        seen(&self.doc, self.scope.as_deref()).unwrap_or(Cow::Borrowed(&self.doc))
    }
}
