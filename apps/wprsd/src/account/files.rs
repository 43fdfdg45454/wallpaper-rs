use super::saved::Files;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::items::reply;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_core::model::{FilesReply, MODEL, PrivateFile};
use wprs_library::{Item, Kind};
use wprs_sync::{Ids, View};

pub fn opted(files: &Files, uid: &str, path: &Path) -> bool {
    let chosen = files.items.get(uid).copied();
    Kind::detect(path).is_some()
        && chosen.unwrap_or_else(|| files.folders.iter().any(|folder| path.starts_with(folder)))
}

pub fn blob_field(files: &Files, ids: &Ids, item: &Item, base: Option<&View>) -> Option<(String, Value)> {
    let stored = files.stored.get(&item.uid).filter(|s| s.done && opted(files, &item.uid, &item.path))?;
    let id = ids.stored(stored.hash.as_bytes());
    let same = base.and_then(|b| b.get("blob")).is_some_and(|b| b["id"] == id.as_str());
    let name = item.path.file_name()?.to_string_lossy().into_owned();
    let blob = json!({"id": id, "hash": stored.hash, "size": stored.size, "name": name});
    (!same).then(|| ("blob".to_owned(), blob))
}

fn hex(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Daemon {
    pub fn remote_file(&self, id: &str) -> Option<Value> {
        let s = self.saved();
        let (_, doc) = s.replica.bases.get(&format!("library/{id}"))?;
        doc.view()?.get("blob").cloned()
    }

    pub fn share_files(self: &Arc<Self>, targets: &[String], share: bool) -> Result<u64> {
        self.require_ready()?;
        if self.saved().encrypted != Some(true) {
            return Err(failed("private files sync only with end-to-end encryption"));
        }
        if targets.is_empty() || targets.len() > 1000 {
            return Err(invalid("give 1 to 1000 items or folders"));
        }
        let (mut folders, mut items, mut remote) = (Vec::new(), Vec::new(), Vec::new());
        for target in targets.iter().map(|t| t.trim()) {
            let blob = self.remote_file(target).filter(|_| hex(target) && !share);
            if Path::new(target).is_absolute() {
                let dir = Path::new(target).canonicalize().ok().filter(|d| d.is_dir());
                folders.push(dir.ok_or_else(|| invalid(format!("{target} is not a folder")))?);
            } else if let Some(blob) = blob {
                remote.push((target.to_owned(), blob["id"].as_str().unwrap_or_default().to_owned()));
            } else {
                let uid = self.store.read(|lib| Ok::<_, Error>(lib.item(target)?.uid));
                items.push(uid.map_err(|_| not_found(format!("no item or private file {target}")))?);
            }
        }
        self.update(|s| {
            for dir in folders {
                if share {
                    s.files.folders.insert(dir)
                } else {
                    s.files.folders.remove(&dir)
                };
            }
            s.files.items.extend(items.into_iter().map(|uid| (uid, share)));
            s.files.dropped.extend(remote);
        })?;
        self.account.wake.notify_one();
        Ok(self.files_job())
    }

    pub fn list_files(&self) -> Result<String> {
        let serial = self.changes.serial();
        let s = self.saved();
        let local = self.store.read(|lib| lib.hashed())?;
        let item =
            |key: &str| s.files.here.get(key).and_then(|uid| local.iter().find(|(i, _)| i.uid == *uid));
        let mut files: Vec<PrivateFile> = Vec::new();
        for (key, (_, doc)) in s.replica.bases.iter().filter(|(key, _)| key.starts_with("library/")) {
            let Some(blob) = doc.view().and_then(|v| v.get("blob").cloned()) else { continue };
            let id = key.trim_start_matches("library/").to_owned();
            let here = item(&id);
            files.push(PrivateFile {
                name: blob["name"].as_str().unwrap_or_default().to_owned(),
                size: blob["size"].as_u64().unwrap_or_default(),
                item: here.map(|(i, _)| i.uid.clone()),
                shared: here.is_some_and(|(i, _)| opted(&s.files, &i.uid, &i.path)),
                stored: true,
                id,
            });
        }
        let waiting = local.iter().filter(|(i, _)| opted(&s.files, &i.uid, &i.path));
        let waiting = waiting.filter(|(i, _)| !files.iter().any(|f| f.item.as_ref() == Some(&i.uid)));
        let waiting: Vec<PrivateFile> = waiting
            .map(|(i, _)| PrivateFile {
                id: String::new(),
                name: i.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                size: i.size,
                item: Some(i.uid.clone()),
                shared: true,
                stored: false,
            })
            .collect();
        files.extend(waiting);
        let folders = s.files.folders.iter().map(|p| p.display().to_string()).collect();
        reply(FilesReply { model: MODEL, serial, folders, files })
    }
}
