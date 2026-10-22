pub use super::views::Local;
use super::views::{item_view, link, references, settings};
use crate::daemon::Daemon;
use crate::error::{Error, Result};
use crate::jobs::now;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use wprs_sync::{Ids, Replica, View};

pub const DEVICE_KEYS: [&str; 3] = ["playback.volume", "playback.muted", "library.thumb_cache_mb"];
pub const OBJECTS: [&str; 3] = ["playlists/", "settings/profile_", "settings/preset_"];

type Links = BTreeMap<String, String>;

impl Daemon {
    pub fn snapshot(
        &self,
        ids: &Ids,
        replica: &Replica,
        links: &mut Links,
        present: &BTreeSet<String>,
    ) -> Result<Local> {
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
        let mut local = Local::default();
        for (item, hash) in hashed {
            let package = installed.iter().find(|i| i.path == item.path);
            let key = package.map_or_else(|| ids.blob(&hash), |p| ids.library(&p.repo_id, &p.id));
            local.keys.insert(item.uid.clone(), key.clone());
            let uids = local.items.entry(key.clone()).or_default();
            uids.push(item.uid.clone());
            let record = format!("library/{key}");
            match (uids.len(), base(&record)) {
                (1, Some(view)) if !present.contains(&key) => {
                    local.fresh.insert(key);
                    local.views.insert(record, view);
                }
                (1, _) => drop(local.views.insert(record, item_view(&item))),
                _ => {}
            }
        }
        for (info, items) in playlists {
            let key = link(links, ids, "playlist", &info.uid);
            local.views.insert(key.clone(), local.playlist(&info, &items, base(&key)));
        }
        for preset in presets {
            let key = link(links, ids, "preset", &preset.uid);
            let mut view =
                View::from([("name".to_owned(), json!(preset.name)), ("value".to_owned(), preset.value)]);
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
                ("name", name),
                ("outputs", outputs),
                ("options", profile.options),
                ("auto", profile.auto),
                ("priority", priority),
            ];
            local.views.insert(key, fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect());
        }
        let unknown = |view: &View| view.get("item").and_then(Value::as_str).is_some_and(|i| !local.known(i));
        let objects =
            replica.bases.iter().filter(|(key, _)| OBJECTS.iter().any(|kind| key.starts_with(kind)));
        let missing: Vec<(String, View, bool)> = objects
            .filter(|(key, _)| !local.views.contains_key(*key))
            .filter_map(|(key, (_, doc))| Some((key.clone(), doc.view()?, links.contains_key(key))))
            .filter(|(key, view, linked)| !linked || (key.starts_with("settings/preset_") && unknown(view)))
            .collect();
        for (key, view, linked) in missing {
            if !linked {
                local.unlinked.insert(key.clone());
            }
            local.views.insert(key, view);
        }
        local.views.extend(settings(&self.state().config.options(), replica));
        Ok(local)
    }
}
