use super::files::{blob_field, opted};
use super::presets::synced_options;
use crate::daemon::Daemon;
use crate::error::{Error, Result};
use crate::jobs::now;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub use wprs_sync::Local;
use wprs_sync::{
    DEVICE_KEYS, Ids, Kept, Replica, View, edit_fields, item_view, link, object, preserve, references,
    settings,
};

type Links = BTreeMap<String, String>;

impl Daemon {
    pub fn shared_edit(&self, item: &wprs_library::Item) -> Option<wprs_core::Edit> {
        let edits = self.store.data.join("edits");
        let found =
            (item.kind == wprs_library::Kind::Image).then(|| wprs_core::edit::load(&item.path, &edits));
        found?.ok()?.map(|(_, edit)| edit)
    }

    pub fn snapshot(
        &self,
        ids: &Ids,
        replica: &Replica,
        links: &mut Links,
        present: &BTreeSet<String>,
        kept: &mut Kept,
    ) -> Result<Local> {
        let (files, encrypted) = {
            let s = self.saved();
            (s.files.clone(), s.encrypted == Some(true))
        };
        let installed = self.state().installed.clone();
        let (hashed, playlists, profiles, presets) = self.store.read(|lib| {
            let lists = lib.playlists()?.into_iter().map(|p| {
                let items = if p.query.is_none() { lib.playlist_items(&p.uid, now())? } else { Vec::new() };
                Ok::<_, wprs_library::Error>((p, items.into_iter().map(|i| i.uid).collect::<Vec<_>>()))
            });
            let lists = lists.collect::<Result<Vec<_>, _>>()?;
            Ok::<_, Error>((lib.hashed()?, lists, lib.profiles()?, lib.all_presets()?))
        })?;
        let base = |key: &str| replica.bases.get(key).and_then(|(_, doc)| doc.view());
        let (mut local, mut dropped) =
            (Local::default(), files.dropped.keys().cloned().collect::<BTreeSet<_>>());
        for (item, hash) in hashed {
            let package = installed.iter().find(|i| i.path == item.path);
            let key = package.map_or_else(|| ids.blob(&hash), |p| ids.library(&p.repo_id, &p.id));
            local.keys.insert(item.uid.clone(), key.clone());
            let uids = local.items.entry(key.clone()).or_default();
            uids.push(item.uid.clone());
            let record = format!("library/{key}");
            if wprs_sync::deleted_before(replica, &record, item.added_at) {
                continue;
            }
            if files.stored.get(&item.uid).is_some_and(|s| s.done) && !opted(&files, &item.uid, &item.path) {
                dropped.insert(key.clone());
            }
            match (uids.len(), base(&record)) {
                (1, Some(view)) if !present.contains(&key) => {
                    local.fresh.insert(key);
                    local.views.insert(record, view);
                }
                (1, old) => {
                    let mut view = item_view(&item);
                    view.extend(blob_field(&files, ids, &item, old.as_ref()).filter(|_| package.is_none()));
                    view.extend(edit_fields(self.shared_edit(&item).as_ref(), old.as_ref()));
                    local.views.insert(record, view);
                }
                _ => {}
            }
        }
        for (info, items) in playlists {
            let key = link(links, ids, "playlist", &info.uid);
            local.views.insert(key.clone(), local.playlist(&info, &items, base(&key)));
        }
        for preset in presets {
            let key = link(links, ids, "preset", &preset.uid);
            let fields = [("kind", json!("preset")), ("name", json!(preset.name)), ("value", preset.value)];
            let mut view: View = fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
            let item = preset.item.and_then(|uid| local.keys.get(&uid));
            view.extend(item.map(|key| ("item".to_owned(), json!(key))));
            local.views.insert(key, view);
        }
        for profile in profiles {
            let key = link(links, ids, "profile", &profile.uid);
            let to = |field: &str, uid: &str| match field {
                "item" => local.keys.get(uid).cloned(),
                _ => links.iter().find(|(_, linked)| *linked == uid).map(|(k, _)| k.clone()),
            };
            let known = |field: &str, key: &str| {
                if field == "item" { local.known(key) } else { links.contains_key(key) }
            };
            let old = base(&key).and_then(|view| view.get("outputs").cloned());
            let outputs = references(&profile.outputs, old.as_ref(), &to, &known);
            let (name, priority) = (json!(profile.name), json!(profile.priority));
            let fields = [
                ("kind", json!("profile")),
                ("name", name),
                ("outputs", outputs),
                ("options", synced_options(&profile.options, None)),
                ("auto", profile.auto),
                ("priority", priority),
            ];
            local.views.insert(key, fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect());
        }
        let unknown = |view: &View| view.get("item").and_then(Value::as_str).is_some_and(|i| !local.known(i));
        let objects = replica.bases.iter().filter(|(key, (_, doc))| object(key, Some(doc)));
        let missing: Vec<(String, View, bool)> = objects
            .filter(|(key, _)| !local.views.contains_key(*key))
            .filter_map(|(key, (_, doc))| Some((key.clone(), doc.view()?, links.contains_key(key))))
            .filter(|(_, view, linked)| {
                !linked || (view.get("kind") == Some(&json!("preset")) && unknown(view))
            })
            .collect();
        for (key, view, linked) in missing {
            if !linked {
                local.unlinked.insert(key.clone());
            }
            local.views.insert(key, view);
        }
        local.views.extend(settings(&self.state().config.options(), replica, ids, &DEVICE_KEYS));
        self.repo_views(ids, replica, present, &mut local, kept)?;
        if encrypted {
            self.secret_views(ids, replica, &mut local)?;
        }
        for key in files.dropped.keys().map(|key| format!("library/{key}")) {
            if let Some(view) = base(&key).filter(|_| !local.views.contains_key(&key)) {
                local.views.insert(key, view.into_iter().filter(|(field, _)| field != "blob").collect());
            }
        }
        preserve(&mut local.views, replica, kept, &dropped);
        Ok(local)
    }
}
