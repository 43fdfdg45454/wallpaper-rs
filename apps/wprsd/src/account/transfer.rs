use super::files::opted;
use super::saved::Stored;
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed, not_found};
use crate::jobs::{Ctl, now};
use crate::store::private_dir;
use reqwest::Method;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::model::JobKind;
use wprs_sync::{Api, SEALED, SEGMENT, open_segment, seal_segment, sealed_size};

const BATCH: u64 = 64;

fn blob_path(id: &str, offset: u64) -> String {
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

fn place(dir: &Path, name: &str, part: &Path) -> Result<PathBuf> {
    let (stem, ext) = name.rsplit_once('.').filter(|(stem, _)| !stem.is_empty()).unwrap_or((name, ""));
    for n in 1..10_000 {
        let named = match (n, ext) {
            (1, _) => name.to_owned(),
            (n, "") => format!("{stem} ({n})"),
            (n, ext) => format!("{stem} ({n}).{ext}"),
        };
        match fs::hard_link(part, dir.join(&named)) {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            linked => {
                linked?;
                fs::remove_file(part)?;
                return Ok(dir.join(named));
            }
        }
    }
    Err(failed("no free file name"))
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
        let (mut sent, mut wanted) = (0, Vec::new());
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
                    continue;
                }
            }
            sent += usize::from(fresh);
            let entry = Stored { done: true, ..entry };
            if files.stored.get(&item.uid) != Some(&entry) {
                self.update(|s| drop(s.files.stored.insert(item.uid.clone(), entry)))?;
            }
            wanted.push(id);
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
        Ok(json!({"uploaded": sent}))
    }

    pub fn fetch_file(self: &Arc<Self>, id: &str) -> Result<u64> {
        self.require_ready()?;
        let blob = self.remote_file(id.trim()).ok_or_else(|| not_found(format!("no private file {id}")))?;
        Ok(self.start_job(JobKind::Upload, "fetch", move |d, ctl| d.download(&blob, ctl)))
    }

    fn download(self: &Arc<Self>, blob: &Value, ctl: &Ctl) -> Result<Value> {
        let (api, secrets) = self.account_api(true)?;
        let ids = secrets.sync_key()?.ids();
        let (hash, size) =
            (blob["hash"].as_str().unwrap_or_default(), blob["size"].as_u64().unwrap_or_default());
        let id = ids.stored(hash.as_bytes());
        if blob["id"] != id.as_str() || hash.len() != 64 {
            return Err(failed("the record of this private file is broken"));
        }
        let name: String = blob["name"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_control() && *c != '/')
            .collect();
        let name =
            Some(name.trim_start_matches('.').to_owned()).filter(|n| !n.is_empty()).unwrap_or("file".into());
        let (dir, key) = (self.store.files(), ids.file_key(hash.as_bytes()));
        private_dir(&dir)?;
        let part = dir.join(format!(".part-{}", &id[..16]));
        let (segments, total) = (size.div_ceil(SEGMENT).max(1), sealed_size(size));
        let mut file = OpenOptions::new().create(true).write(true).truncate(false).mode(0o600).open(&part)?;
        let mut index = (file.metadata()?.len() / SEGMENT).min(segments);
        file.set_len(index * SEGMENT)?;
        file.seek(SeekFrom::End(0))?;
        while index < segments {
            ctl.check()?;
            let offset = index * SEALED;
            let length = (BATCH * SEALED).min(total - offset);
            let path = format!("{}&length={length}", blob_path(&id, offset));
            let bytes = block_on(api.get_bytes(&path, length as usize)).map_err(sync_error)?;
            if bytes.len() as u64 != length {
                return Err(failed("the server sent a short piece of the file; fetch again"));
            }
            for chunk in bytes.chunks(SEALED as usize) {
                file.write_all(
                    &open_segment(&key, index, index + 1 == segments, chunk).map_err(sync_error)?,
                )?;
                index += 1;
            }
            ctl.progress(index, segments);
        }
        drop(file);
        if wprs_format::sha256_file(&part)? != hash {
            fs::remove_file(&part)?;
            return Err(failed("the downloaded file does not match its hash"));
        }
        let dest = place(&dir, &name, &part)?;
        let uid = self.store.write(|lib| lib.import_file(&dest, now()))?;
        self.library_changed(vec![uid.clone()]);
        self.analyze_job();
        self.account.wake.notify_one();
        Ok(json!({"item": uid}))
    }
}
