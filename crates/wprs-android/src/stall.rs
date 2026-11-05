use crate::Error;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use wprs_format::{Signer, stall};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Open,
    Add {
        files: Vec<PathBuf>,
        #[serde(default)]
        unlisted: bool,
    },
    List {
        id: String,
        #[serde(default)]
        version: Option<String>,
        listed: bool,
    },
    Feature {
        id: String,
        #[serde(default)]
        note: Option<String>,
        on: bool,
    },
    Publish {
        key: String,
        #[serde(default)]
        password: Option<String>,
        days: u32,
    },
}

#[derive(Deserialize)]
struct Asked {
    dir: PathBuf,
    #[serde(flatten)]
    request: Request,
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let (mut found, mut pending) = (Vec::new(), vec![dir.to_path_buf()]);
    while let Some(at) = pending.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(entry.path()),
                Ok(kind) if kind.is_file() => found.push(entry.path()),
                _ => {}
            }
        }
    }
    found
}

fn sum(path: &Path) -> Option<u64> {
    let mut hasher = DefaultHasher::new();
    std::fs::read(path).ok()?.hash(&mut hasher);
    Some(hasher.finish())
}

fn answer(asked: Asked) -> Result<Value, Error> {
    let dir = asked.dir.as_path();
    let before: BTreeMap<PathBuf, Option<u64>> =
        files(dir).into_iter().map(|p| (p.clone(), sum(&p))).collect();
    let now = crate::library::now();
    let mut state = match asked.request {
        Request::Open => stall::opened(dir)?,
        Request::Add { files, unlisted } => stall::added(dir, &files, unlisted, now)?,
        Request::List { id, version, listed } => stall::listed(dir, &id, version.as_deref(), listed)?,
        Request::Feature { id, note, on } => stall::featured(dir, &id, note.as_deref(), on)?,
        Request::Publish { key, password, days } => {
            stall::published(dir, &Signer::new(&Zeroizing::new(key), password)?, days, now)?
        }
    };
    let changed = files(dir).into_iter().filter(|p| before.get(p).is_none_or(|was| *was != sum(p)));
    let mut changed: Vec<String> =
        changed.filter_map(|p| p.strip_prefix(dir).ok().map(|p| p.to_string_lossy().into_owned())).collect();
    changed.sort();
    state["changed"] = json!(changed);
    Ok(state)
}

pub(crate) fn request(text: &str) -> String {
    let answer = serde_json::from_str(text).map_err(Error::from).and_then(answer);
    answer.unwrap_or_else(|e| e.reply()).to_string()
}
