use super::files::opted;
use super::saved::Stored;
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed};
use crate::jobs::{Ctl, now};
use reqwest::Method;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use wprs_core::model::JobKind;
use wprs_sync::{Api, SEALED, SEGMENT, seal_segment, sealed_size};

pub const BATCH: u64 = 64;

pub fn blob_path(id: &str, offset: u64) -> String {
    format!("/api/v1/sync/blobs/{id}?offset={offset}")
}

fn upload(api: &Api, key: &[u8; 32], path: &Path, size: u64, id: &str, ctl: &Ctl) -> Result<()> {
    let total = sealed_size(size);
    let opened = block_on(api.post(&format!("/api/v1/sync/blobs/{id}"), json!({"size": total})));
    let mut offset = opened.map_err(sync_error)?["received"].as_u64().unwrap_or_default();
    let (segments, mut index) = (size.div_ceil(SEGMENT).max(1), offset / SEALED);
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(index * SEGMENT))?;
    while index < segments {
        ctl.check()?;
        let mut body = Vec::new();
        for _ in 0..BATCH.min(segments - index) {
            let mut plain = Vec::with_capacity(SEGMENT as usize);
            (&mut file).take(SEGMENT).read_to_end(&mut plain)?;
            body.extend(seal_segment(key, index, index + 1 == segments, &plain).map_err(sync_error)?);
            index += 1;
        }
        let answer = block_on(api.put_bytes(&blob_path(id, offset), body)).map_err(sync_error)?;
        offset = answer["received"].as_u64().unwrap_or(total);
        ctl.progress(offset, total);
    }
    Ok(())
}

type Held = HashMap<String, (u64, u64, i64)>;

fn listing(api: &Api) -> Result<Held> {
    let (mut held, mut after) = (Held::new(), String::new());
    loop {
        let page = block_on(api.get(&format!("/api/v1/sync/blobs?after={after}"))).map_err(sync_error)?;
        let items = page["items"].as_array().cloned().unwrap_or_default();
        held.extend(items.iter().filter_map(|i| {
            let found = (i["size"].as_u64()?, i["received"].as_u64()?, i["created_at"].as_i64()?);
            Some((i["id"].as_str()?.to_owned(), found))
        }));
        match items.last().and_then(|i| i["id"].as_str()).filter(|_| page["more"] == true) {
            Some(last) if last > after.as_str() => after = last.to_owned(),
            _ => return Ok(held),
        }
    }
}

impl Daemon {
    pub fn files_job(self: &Arc<Self>) -> u64 {
        match self.jobs.find(JobKind::Upload, "files").into_iter().min() {
            Some(job) => job,
            None => self.start_job(JobKind::Upload, "files", |d, ctl| d.transfer(ctl)),
        }
    }

    fn transfer(self: &Arc<Self>, ctl: &Ctl) -> Result<Value> {
        let (api, secrets) = self.account_api(false)?;
        let ids = secrets.sync_key()?.ids();
        let installed = self.state().installed.clone();
        let (files, replica) = {
            let s = self.saved();
            (s.files.clone(), s.replica.clone())
        };
        let held = listing(&api)?;
        let blob = |key: &str| {
            replica.bases.get(key).and_then(|(_, d)| d.view()?.get("blob")?["id"].as_str().map(String::from))
        };
        let gone = |id: &str| {
            drop(block_on(api.call(Method::DELETE, &format!("/api/v1/sync/blobs/{id}"), None)));
            self.update(|s| {
                s.files.uploaded.remove(id);
            })
        };
        let (mut sent, mut wanted, mut failures) = (0, Vec::new(), Vec::new());
        for (item, qhash) in self.store.read(|lib| lib.hashed())? {
            let key = format!("library/{}", ids.blob(&qhash));
            let stored = files.stored.get(&item.uid).cloned();
            if !opted(&files, &item.uid, &item.path) || installed.iter().any(|i| i.path == item.path) {
                let id = stored.filter(|s| s.done).map(|s| ids.stored(s.hash.as_bytes()));
                if let Some(id) = id.filter(|id| blob(&key).as_ref() != Some(id)) {
                    gone(&id)?;
                    self.update(|s| drop(s.files.stored.remove(&item.uid)))?;
                }
                continue;
            }
            ctl.check()?;
            let one = || -> Result<Option<(bool, String)>> {
                let same = stored.filter(|s| (s.size, s.mtime) == (item.size, item.mtime));
                let fresh = !same.as_ref().is_some_and(|s| s.done);
                let hash = same.map_or_else(|| wprs_format::sha256_file(&item.path), |s| Ok(s.hash))?;
                let id = ids.stored(hash.as_bytes());
                let total = sealed_size(item.size);
                let entry = Stored { hash: hash.clone(), size: item.size, mtime: item.mtime, done: false };
                if held.get(&id).is_none_or(|(size, received, _)| (*size, *received) != (total, total)) {
                    self.update(|s| {
                        s.files.stored.insert(item.uid.clone(), entry.clone());
                        s.files.uploaded.insert(id.clone());
                    })?;
                    upload(&api, &ids.file_key(hash.as_bytes()), &item.path, item.size, &id, ctl)?;
                    let meta = fs::metadata(&item.path)?;
                    if (meta.len(), meta.mtime()) != (item.size, item.mtime) {
                        gone(&id)?;
                        return Ok(None);
                    }
                }
                let entry = Stored { done: true, ..entry };
                if files.stored.get(&item.uid) != Some(&entry) {
                    self.update(|s| drop(s.files.stored.insert(item.uid.clone(), entry)))?;
                }
                Ok(Some((fresh, id)))
            };
            match one() {
                Ok(Some((fresh, id))) => {
                    (sent, wanted) = (sent + usize::from(fresh), [wanted, vec![id]].concat())
                }
                Ok(None) => {}
                Err(e @ crate::error::Error::Cancelled) => return Err(e),
                Err(e) => failures.push(format!("{}: {e}", item.title)),
            }
        }
        for (key, id) in &files.dropped {
            if blob(&format!("library/{key}")).as_ref() != Some(id) {
                gone(id)?;
                self.update(|s| drop(s.files.dropped.remove(key)))?;
            }
        }
        let state = block_on(api.get("/api/v1/sync/state")).map_err(sync_error)?;
        let (fresh, mine) = {
            let s = self.saved();
            let stored = s.files.stored.values().filter(|s| s.done).map(|s| ids.stored(s.hash.as_bytes()));
            (s.replica.clone(), s.files.uploaded.iter().cloned().chain(stored).collect::<Vec<_>>())
        };
        if state["seq"].as_i64() == Some(fresh.seq) {
            let library = fresh.bases.iter().filter(|(key, _)| key.starts_with("library/"));
            let used: Vec<String> = library
                .filter_map(|(_, (_, d))| d.view()?.get("blob")?["id"].as_str().map(String::from))
                .collect();
            let old = |(id, (size, received, at)): (&String, &(u64, u64, i64))| {
                size == received
                    && *at < (now() - 86_400) * 1000
                    && !used.contains(id)
                    && !wanted.contains(id)
            };
            for (id, _) in held.iter().filter(|entry| mine.contains(entry.0) && old(*entry)) {
                gone(id)?;
            }
        }
        if sent > 0 {
            self.account.wake.notify_one();
        }
        if !failures.is_empty() {
            let count = failures.len();
            return Err(failed(format!(
                "{count} private files were not uploaded, the others were: {}",
                failures.join("; ")
            )));
        }
        Ok(json!({"uploaded": sent}))
    }
}
