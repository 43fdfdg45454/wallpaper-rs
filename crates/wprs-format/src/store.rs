use crate::entry::contents;
use crate::repo::hex64;
use crate::{Entry, Error, Result, Revoked, Root, Shard, require};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub url: String,
    pub bases: Vec<String>,
    pub repo_id: String,
    pub root_version: u64,
    pub entry_version: u64,
    pub entry_sha256: String,
    pub entry_timestamp: i64,
    pub max_timestamp: i64,
    pub online_keys: Vec<String>,
    pub stale: bool,
    pub checked: i64,
}

pub(crate) struct Doc<T> {
    pub value: T,
    pub json: Vec<u8>,
    pub sig: String,
}

pub(crate) struct Snapshot {
    pub state: State,
    pub root: Doc<Root>,
    pub entry: Doc<Entry>,
    pub blobs: BTreeMap<String, Vec<u8>>,
    pub shards: Vec<Shard>,
    pub revoked: Revoked,
}

pub(crate) struct Temp {
    path: PathBuf,
    pub file: File,
}

impl Temp {
    pub fn new(dest: &Path, private: bool) -> Result<Temp> {
        let dir = dest.parent().ok_or_else(|| Error::Blocked(dest.into()))?;
        DirBuilder::new().recursive(true).mode(if private { 0o700 } else { 0o755 }).create(dir)?;
        let path = dir.join(format!(".tmp-{}-{}", process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        let mode = if private { 0o600 } else { 0o644 };
        let file = OpenOptions::new().read(true).write(true).create_new(true).mode(mode).open(&path)?;
        Ok(Temp { path, file })
    }

    pub fn keep(self, dest: &Path) -> Result<()> {
        self.file.sync_all()?;
        fs::rename(&self.path, dest)?;
        Ok(File::open(dest.parent().ok_or_else(|| Error::Blocked(dest.into()))?)?.sync_all()?)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) fn place(dest: &Path, private: bool, fill: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let mut tmp = Temp::new(dest, private)?;
    fill(&mut tmp.file)?;
    tmp.keep(dest)
}

pub(crate) fn locked<T>(data: &Path, exclusive: bool, run: impl FnOnce() -> Result<T>) -> Result<T> {
    if exclusive {
        DirBuilder::new().recursive(true).mode(0o700).create(data)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false).mode(0o600);
    let file = match options.open(data.join(".repos.lock")) {
        Err(e) if e.kind() == io::ErrorKind::NotFound && !exclusive => return run(),
        file => file?,
    };
    if exclusive {
        file.lock()?
    } else {
        file.lock_shared()?
    }
    run()
}

pub(crate) fn write(dest: &Path, bytes: &[u8], private: bool) -> Result<()> {
    place(dest, private, |file| Ok(file.write_all(bytes)?))
}

pub(crate) fn json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub(crate) fn optional<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn repo_dir(data: &Path, repo_id: &str) -> Result<PathBuf> {
    require(hex64(repo_id), "repo_id", "expected 64 lowercase hex digits")?;
    Ok(data.join("repos").join(repo_id))
}

pub(crate) fn load(data: &Path, repo_id: &str) -> Result<Snapshot> {
    let dir = repo_dir(data, repo_id)?;
    let read = |name: &str| -> Result<(Vec<u8>, String)> {
        Ok((fs::read(dir.join(name))?, fs::read_to_string(dir.join(format!("{name}.minisig")))?))
    };
    let ((root_json, root_sig), (entry_json, entry_sig)) = (read("root.json")?, read("entry.json")?);
    let root = Doc { value: serde_json::from_slice(&root_json)?, json: root_json, sig: root_sig };
    let entry: Doc<Entry> =
        Doc { value: serde_json::from_slice(&entry_json)?, json: entry_json, sig: entry_sig };
    let blobs = entry.value.shards.iter().chain([&entry.value.revoked]).map(|blob| {
        let bytes = fs::read(dir.join("blobs").join(format!("{}.json", blob.sha256)))?;
        blob.matches(&bytes).map(|()| (blob.sha256.clone(), bytes))
    });
    let blobs = blobs.collect::<Result<BTreeMap<_, _>>>()?;
    let (shards, revoked) = contents(&entry.value, repo_id, &blobs)?;
    Ok(Snapshot { state: json(&dir.join("state.json"))?, root, entry, blobs, shards, revoked })
}

pub(crate) fn commit(data: &Path, snap: &Snapshot) -> Result<()> {
    let dir = repo_dir(data, &snap.state.repo_id)?;
    let (root, entry) = (&snap.root, &snap.entry);
    let docs = [("root.json", &root.json, &root.sig), ("entry.json", &entry.json, &entry.sig)];
    for (name, json, sig) in docs {
        write(&dir.join(name), json, true)?;
        write(&dir.join(format!("{name}.minisig")), sig.as_bytes(), true)?;
    }
    for (sha, bytes) in &snap.blobs {
        let path = dir.join("blobs").join(format!("{sha}.json"));
        if !path.exists() {
            write(&path, bytes, true)?;
        }
    }
    for item in fs::read_dir(dir.join("blobs"))? {
        let name = item?.file_name().to_string_lossy().into_owned();
        if !name.strip_suffix(".json").is_some_and(|sha| snap.blobs.contains_key(sha)) {
            fs::remove_file(dir.join("blobs").join(name))?;
        }
    }
    write(&dir.join("state.json"), &serde_json::to_vec_pretty(&snap.state)?, true)
}
