use super::View;
use crate::{Error, chosen, props, store};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use wprs_scene::{Control, Mode};

const ORIGINS: &str = "web-origins.json";
const USER: &str = ":user";

static SALT: OnceLock<RandomState> = OnceLock::new();
static LOCK: Mutex<()> = Mutex::new(());

fn token(host: &str, prop: &str, path: &Path) -> String {
    let hash = format!("{:016x}", SALT.get_or_init(RandomState::new).hash_one(path));
    format!("https://{host}/{USER}/{prop}/{}", wprs_format::page::token_name(path, &hash))
}

pub(super) fn user(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix(USER)?.strip_prefix('/')?.split_once('/')
}

pub(super) type Folders = BTreeMap<String, Vec<String>>;

pub(super) fn values(view: &mut View) -> (Map<String, Value>, Folders) {
    let (pack, mut folders) = (view.pack.clone(), Folders::new());
    let host = view.host.clone();
    let mut values: Map<String, Value> = props::values(&pack).into_iter().collect();
    for (id, value) in values.iter_mut() {
        let path = value.as_str().filter(|v| v.starts_with('/')).map(PathBuf::from);
        let control = pack.properties.get(id).map(|p| &p.control);
        *value = match (control, path) {
            (Some(Control::File { .. }), Some(path)) => {
                let token = token(&host, id, &path);
                view.files.insert(token.clone(), path);
                json!(token)
            }
            (Some(Control::Folder { mode, .. }), Some(dir)) => {
                let files = chosen::listed(&dir).into_iter().map(|p| (token(&host, id, &p), p));
                let files: Vec<(String, PathBuf)> = files.collect();
                view.files.extend(files.iter().cloned());
                let all = *mode == Mode::All;
                folders.insert(
                    format!("{}{id}", if all { "*" } else { "" }),
                    files.into_iter().map(|f| f.0).collect(),
                );
                json!(format!("https://{host}/{USER}/{id}/"))
            }
            (_, Some(_)) => Value::Null,
            _ => value.take(),
        };
    }
    (values, folders)
}

pub(super) fn listed<'a>(folders: &'a Folders, prop: &str, every: bool) -> Option<&'a Vec<String>> {
    let all = folders.get(&format!("*{prop}"));
    if every { all } else { all.or_else(|| folders.get(prop)) }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Origins {
    generations: BTreeMap<String, u64>,
    pending: Vec<String>,
}

fn file() -> Result<PathBuf, Error> {
    Ok(store::dir()?.join(ORIGINS))
}

fn origins() -> Origins {
    let read = file().ok().and_then(|path| fs::read(path).ok());
    read.and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn keep(origins: &Origins) -> Result<(), Error> {
    store::write(&file()?, &serde_json::to_vec_pretty(origins)?)
}

pub(crate) fn count(id: &str) -> u64 {
    let _held = LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    origins().generations.get(id).copied().unwrap_or(0)
}

pub(crate) fn clear(id: &str) -> Result<(), Error> {
    let _held = LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut all = origins();
    let generation = all.generations.entry(id.into()).or_default();
    all.pending.push(format!("https://{}", super::host(id, *generation)));
    *generation += 1;
    keep(&all)?;
    store::changed();
    Ok(())
}

pub(crate) fn cleared() -> Result<Value, Error> {
    let _held = LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut all = origins();
    let pending = std::mem::take(&mut all.pending);
    if !pending.is_empty() {
        keep(&all)?;
    }
    Ok(json!(pending))
}
