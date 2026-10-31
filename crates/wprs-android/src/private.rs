use crate::said;
use crate::state::{read, update};
use crate::{Error, library};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use wprs_library::{Item, Kind};
use wprs_sync::{Ids, Local, Replica, View};

mod transfer;

pub(crate) use transfer::{fetch, transfer};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Stored {
    pub(crate) hash: String,
    pub(crate) size: u64,
    pub(crate) done: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Files {
    pub(crate) items: BTreeSet<String>,
    pub(crate) stored: BTreeMap<String, Stored>,
    pub(crate) dropped: BTreeMap<String, String>,
    pub(crate) here: BTreeMap<String, String>,
}

impl Files {
    pub(crate) fn idle(&self) -> bool {
        self.items.is_empty() && self.stored.is_empty() && self.dropped.is_empty()
    }
}

pub(crate) fn name(item: &Item) -> String {
    let extension = item.path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let title: String = item.title.chars().filter(|c| !c.is_control() && *c != '/').collect();
    let title = Some(title.trim().to_owned()).filter(|t| !t.is_empty()).unwrap_or_else(|| "file".into());
    if extension.is_empty() { title } else { format!("{title}.{extension}") }
}

pub(crate) fn blob_field(
    files: &Files,
    ids: &Ids,
    item: &Item,
    base: Option<&View>,
) -> Option<(String, Value)> {
    let stored = files.stored.get(&item.uid).filter(|s| s.done && files.items.contains(&item.uid))?;
    let id = ids.stored(stored.hash.as_bytes());
    let same = base.and_then(|b| b.get("blob")).is_some_and(|b| b["id"] == id.as_str());
    let blob = json!({"id": id, "hash": stored.hash, "size": stored.size, "name": name(item)});
    (!same).then(|| ("blob".to_owned(), blob))
}

pub(crate) fn dropped(files: &Files, local: &Local) -> BTreeSet<String> {
    let gone = files.stored.iter().filter(|(uid, s)| s.done && !files.items.contains(*uid));
    let keys = gone.filter_map(|(uid, _)| local.keys.get(uid).cloned());
    keys.chain(files.dropped.keys().cloned()).collect()
}

pub(crate) fn unblob(files: &Files, replica: &Replica, local: &mut Local) {
    for key in files.dropped.keys().map(|key| format!("library/{key}")) {
        let base = replica.bases.get(&key).and_then(|(_, doc)| doc.view());
        if let Some(view) = base.filter(|_| !local.views.contains_key(&key)) {
            local.views.insert(key, view.into_iter().filter(|(field, _)| field != "blob").collect());
        }
    }
}

fn remote(replica: &Replica, key: &str) -> Option<Value> {
    replica.bases.get(&format!("library/{key}"))?.1.view()?.get("blob").cloned()
}

fn hex(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn share(targets: &[String], share: bool) -> Result<Value, Error> {
    if read(|s| s.encrypted) != Some(true) {
        return Err(said(
            "android.files_encryption",
            "private files sync only with end-to-end encryption",
            &[],
        ));
    }
    let (mut items, mut gone) = (Vec::new(), Vec::new());
    for target in targets.iter().map(|t| t.trim()) {
        match read(|s| remote(&s.replica, target)).filter(|_| hex(target) && !share) {
            Some(blob) => gone.push((target.to_owned(), blob["id"].as_str().unwrap_or_default().to_owned())),
            None => items.push(library::with(|lib| Ok(lib.item(target)?.uid))?),
        }
    }
    update(|s| {
        match share {
            true => s.files.items.extend(items),
            false => s.files.items.retain(|uid| !items.contains(uid)),
        }
        s.files.dropped.extend(gone);
    })?;
    Ok(list())
}

pub(crate) fn list() -> Value {
    let items: Vec<Item> =
        library::with(|lib| Ok(lib.hashed()?)).unwrap_or_default().into_iter().map(|(i, _)| i).collect();
    read(|s| {
        let here = |key: &str| s.files.here.get(key).and_then(|uid| items.iter().find(|i| i.uid == *uid));
        let library = s.replica.bases.iter().filter(|(key, _)| key.starts_with("library/"));
        let stored =
            library.filter_map(|(key, (_, doc))| Some((key.trim_start_matches("library/"), doc.view()?)));
        let mut files: Vec<Value> = stored
            .filter_map(|(id, view)| {
                let blob = view.get("blob")?;
                let item = here(id);
                Some(json!({"id": id, "name": blob["name"], "size": blob["size"], "stored": true,
                    "item": item.map(|i| &i.uid), "shared": item.is_some_and(|i| s.files.items.contains(&i.uid))}))
            })
            .collect();
        let waiting = items.iter().filter(|i| s.files.items.contains(&i.uid) && i.kind != Kind::Package);
        let waiting = waiting.filter(|i| !files.iter().any(|f| f["item"] == i.uid.as_str()));
        let waiting: Vec<Value> = waiting
            .map(|i| json!({"id": "", "name": name(i), "size": i.size, "stored": false, "item": i.uid, "shared": true}))
            .collect();
        files.extend(waiting);
        json!({"files": files})
    })
}
