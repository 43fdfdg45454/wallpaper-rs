use crate::contents::{files, used};
use crate::history::History;
use crate::ops::Op;
use crate::project::Project;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const MESHES: &str = "meshes/";

fn strings(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::String(s) if s.starts_with(MESHES) => drop(out.insert(s.clone())),
        Value::Array(list) => list.iter().for_each(|x| strings(x, out)),
        Value::Object(map) => map.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

pub(crate) fn named(history: &History) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let steps = history.done.iter().chain(&history.undone);
    for op in steps.flat_map(|s| s.undo.iter().chain(&s.redo)) {
        if let Op::Add { value, .. } | Op::Replace { value, .. } = op {
            strings(value, &mut out);
        }
    }
    out
}

pub(crate) fn spare(dir: &Path, doc: &Value) -> Vec<String> {
    let used = used(doc);
    let list = files(dir).into_iter().map(|(p, _)| p);
    list.filter(|p| p.starts_with(MESHES) && p.ends_with(".glb") && !used.contains(p)).collect()
}

fn shift(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(up) = to.parent() {
        fs::create_dir_all(up)?;
    }
    fs::rename(from, to).or_else(|_| fs::copy(from, to).and_then(|_| fs::remove_file(from)))
}

impl Project {
    pub(crate) fn tuck(&self) {
        let Ok(stash) = crate::session::stash(&self.dir) else { return };
        let named = named(&self.history);
        for p in spare(&self.dir, &self.doc).into_iter().filter(|p| named.contains(p)) {
            shift(&self.dir.join(&p), &stash.join(&p)).ok();
        }
        for (p, _) in files(&stash).into_iter().filter(|(p, _)| !named.contains(p)) {
            fs::remove_file(stash.join(p)).ok();
        }
    }

    pub(crate) fn bring_back(&self) {
        let Ok(stash) = crate::session::stash(&self.dir) else { return };
        for p in used(&self.doc).into_iter().filter(|p| p.starts_with(MESHES)) {
            if !self.dir.join(&p).exists() && stash.join(&p).is_file() {
                shift(&stash.join(&p), &self.dir.join(&p)).ok();
            }
        }
    }
}
