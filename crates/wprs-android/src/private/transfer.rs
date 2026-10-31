use super::Stored;
use crate::said;
use crate::state::{CANCEL, read, update};
use crate::{Error, library, store};
use reqwest::Method;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::Ordering::Relaxed;
use wprs_library::Kind;
use wprs_sync::{Api, SEALED, SEGMENT, Secrets, open_segment, seal_segment, sealed_size};

const BATCH: u64 = 64;

fn path(id: &str, offset: u64) -> String {
    format!("/api/v1/sync/blobs/{id}?offset={offset}")
}

fn stopped() -> Result<(), Error> {
    match CANCEL.load(Relaxed) {
        true => Err(said("android.stopped", "stopped", &[])),
        false => Ok(()),
    }
}

async fn listing(api: &Api) -> Result<HashMap<String, (u64, u64)>, Error> {
    let (mut held, mut after) = (HashMap::new(), String::new());
    loop {
        let page = api.get(&format!("/api/v1/sync/blobs?after={after}")).await?;
        let items = page["items"].as_array().cloned().unwrap_or_default();
        held.extend(items.iter().filter_map(|i| {
            Some((i["id"].as_str()?.to_owned(), (i["size"].as_u64()?, i["received"].as_u64()?)))
        }));
        match items.last().and_then(|i| i["id"].as_str()).filter(|_| page["more"] == true) {
            Some(last) if last > after.as_str() => after = last.to_owned(),
            _ => return Ok(held),
        }
    }
}

async fn upload(api: &Api, key: &[u8; 32], file: &std::path::Path, size: u64, id: &str) -> Result<(), Error> {
    let total = sealed_size(size);
    let opened = api.post(&format!("/api/v1/sync/blobs/{id}"), json!({"size": total})).await?;
    let mut offset = opened["received"].as_u64().unwrap_or_default();
    let (segments, mut index) = (size.div_ceil(SEGMENT).max(1), offset / SEALED);
    let mut file = File::open(file)?;
    file.seek(SeekFrom::Start(index * SEGMENT))?;
    while index < segments {
        stopped()?;
        let mut body = Vec::new();
        for _ in 0..BATCH.min(segments - index) {
            let mut plain = Vec::with_capacity(SEGMENT as usize);
            (&mut file).take(SEGMENT).read_to_end(&mut plain)?;
            body.extend(seal_segment(key, index, index + 1 == segments, &plain)?);
            index += 1;
        }
        offset = api.put_bytes(&path(id, offset), body).await?["received"].as_u64().unwrap_or(total);
    }
    Ok(())
}

pub(crate) async fn transfer(api: &Api, secrets: &Secrets) -> Result<usize, Error> {
    let ids = secrets.sync_key()?.ids();
    let (files, replica) = read(|s| (s.files.clone(), s.replica.clone()));
    let held = listing(api).await?;
    let blob = |key: &str| {
        replica.bases.get(key).and_then(|(_, d)| d.view()?.get("blob")?["id"].as_str().map(String::from))
    };
    let gone =
        async |id: &str| drop(api.call(Method::DELETE, &format!("/api/v1/sync/blobs/{id}"), None).await);
    let (mut sent, mut failures) = (0, Vec::new());
    for (item, qhash) in library::with(|lib| Ok(lib.hashed()?))? {
        let key = format!("library/{}", ids.blob(&qhash));
        let stored = files.stored.get(&item.uid).cloned();
        if !files.items.contains(&item.uid) || item.kind == Kind::Package {
            let id = stored.filter(|s| s.done).map(|s| ids.stored(s.hash.as_bytes()));
            if let Some(id) = id.filter(|id| blob(&key).as_ref() != Some(id)) {
                gone(&id).await;
                update(|s| drop(s.files.stored.remove(&item.uid)))?;
            }
            continue;
        }
        stopped()?;
        let one = async || -> Result<bool, Error> {
            let same = stored.filter(|s| s.size == item.size);
            let fresh = !same.as_ref().is_some_and(|s| s.done);
            let hash = match same {
                Some(s) => s.hash,
                None => wprs_format::sha256_file(&item.path)?,
            };
            let id = ids.stored(hash.as_bytes());
            let entry = Stored { hash: hash.clone(), size: item.size, done: false };
            let total = sealed_size(item.size);
            if held.get(&id).is_none_or(|found| *found != (total, total)) {
                update(|s| drop(s.files.stored.insert(item.uid.clone(), entry.clone())))?;
                upload(api, &ids.file_key(hash.as_bytes()), &item.path, item.size, &id).await?;
            }
            update(|s| drop(s.files.stored.insert(item.uid.clone(), Stored { done: true, ..entry })))?;
            Ok(fresh)
        };
        match one().await {
            Ok(fresh) => sent += usize::from(fresh),
            Err(e) if CANCEL.load(Relaxed) => return Err(e),
            Err(e) => failures.push(format!("{}: {e}", item.title)),
        }
    }
    for (key, id) in &files.dropped {
        if blob(&format!("library/{key}")).as_ref() != Some(id) {
            gone(id).await;
            update(|s| drop(s.files.dropped.remove(key)))?;
        }
    }
    match failures.len() {
        0 => Ok(sent),
        count => Err(said(
            "android.files_not_uploaded",
            "%1 private files were not uploaded, the others were: %2",
            &[&count, &failures.join("; ")],
        )),
    }
}

pub(crate) async fn fetch(api: &Api, secrets: &Secrets, key: &str) -> Result<Value, Error> {
    let found = read(|s| super::remote(&s.replica, key.trim()));
    let blob = found.ok_or_else(|| said("android.no_private_file", "no private file %1", &[&key]))?;
    let ids = secrets.sync_key()?.ids();
    let (hash, size) = (blob["hash"].as_str().unwrap_or_default(), blob["size"].as_u64().unwrap_or_default());
    let id = ids.stored(hash.as_bytes());
    if blob["id"] != id.as_str() || hash.len() != 64 {
        return Err(said("android.private_file_broken", "the record of this private file is broken", &[]));
    }
    let name: String =
        blob["name"].as_str().unwrap_or("file").chars().filter(|c| !c.is_control() && *c != '/').collect();
    let file_key = ids.file_key(hash.as_bytes());
    let part = store::dir()?.join(format!(".part-{}", &id[..16]));
    let (segments, total) = (size.div_ceil(SEGMENT).max(1), sealed_size(size));
    let mut file = std::fs::OpenOptions::new().create(true).write(true).truncate(false).open(&part)?;
    let mut index = (file.metadata()?.len() / SEGMENT).min(segments);
    file.set_len(index * SEGMENT)?;
    file.seek(SeekFrom::End(0))?;
    while index < segments {
        stopped()?;
        let offset = index * SEALED;
        let length = (BATCH * SEALED).min(total - offset);
        let bytes = api.get_bytes(&format!("{}&length={length}", path(&id, offset)), length as usize).await?;
        if bytes.len() as u64 != length {
            return Err(said(
                "android.private_file_short",
                "the server sent a short piece of the file; fetch again",
                &[],
            ));
        }
        for chunk in bytes.chunks(SEALED as usize) {
            file.write_all(&open_segment(&file_key, index, index + 1 == segments, chunk)?)?;
            index += 1;
        }
    }
    drop(file);
    let same = wprs_format::sha256_file(&part)? == hash;
    let stored =
        same.then(|| File::open(&part).map_err(Error::from).and_then(|mut f| store::store(&mut f, &name)));
    drop(std::fs::remove_file(&part));
    let (uid, _) = stored.ok_or_else(|| {
        said("android.private_file_hash", "the downloaded file does not match its hash", &[])
    })??;
    store::changed();
    Ok(json!({"item": uid}))
}
