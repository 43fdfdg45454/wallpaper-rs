use super::apply::hold;
use super::views::{Kept, Local};
use crate::daemon::Daemon;
use crate::error::{Error, Result};
use crate::jobs::now;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_library::{Library, PlaylistInfo, Rules};
use wprs_sync::{Ids, View};

pub type Links = BTreeMap<String, String>;

pub fn text(view: &View, field: &str, fallback: &str) -> String {
    view.get(field).and_then(Value::as_str).unwrap_or(fallback).to_owned()
}

pub fn playlist_rules(view: &View, current: &Rules) -> (Rules, BTreeMap<String, Value>) {
    let shown = serde_json::to_value(current).unwrap_or_default();
    let (mut merged, mut held) =
        (serde_json::to_value(Rules::default()).unwrap_or_default(), BTreeMap::new());
    for (field, value) in view.iter().filter_map(|(k, v)| Some((k.strip_prefix("r:")?, v))) {
        if merged.get(field).is_none() {
            continue;
        }
        let mut tried = merged.clone();
        tried[field] = value.clone();
        if serde_json::from_value::<Rules>(tried.clone()).is_ok() {
            merged = tried;
        } else {
            let old = shown.get(field).cloned().unwrap_or_default();
            merged[field] = old.clone();
            held.insert(format!("r:{field}"), old);
        }
    }
    (serde_json::from_value(merged).unwrap_or_else(|_| current.clone()), held)
}

pub fn keep_name(kept: &mut Kept, key: &str, shown: &str, received: &str) {
    let fields = kept.entry(key.to_owned()).or_default();
    match shown == received {
        true => drop(fields.remove("name")),
        false => drop(fields.insert("name".into(), json!(shown))),
    }
    if fields.is_empty() {
        kept.remove(key);
    }
}

pub fn renamed<T>(
    name: &str,
    mut create: impl FnMut(&str) -> wprs_library::Result<T>,
) -> wprs_library::Result<T> {
    match create(name) {
        Err(wprs_library::Error::Exists(..)) => create(&format!("{name} (2)")),
        done => done,
    }
}

impl Daemon {
    pub fn linked_uid(
        &self,
        links: &Links,
        key: &str,
        exists: impl Fn(&Library, &str) -> bool,
    ) -> Option<String> {
        let uid = links.get(key)?.clone();
        self.store.read(|lib| Ok::<_, Error>(exists(lib, &uid))).unwrap_or(false).then_some(uid)
    }

    pub fn apply_playlist(
        &self,
        local: &Local,
        key: &str,
        view: Option<View>,
        links: &mut Links,
        kept: &mut Kept,
    ) -> Result<()> {
        let uid = self.linked_uid(links, key, |lib, uid| lib.playlist(uid).is_ok());
        let Some(view) = view else {
            links.remove(key);
            return uid.map_or(Ok(()), |uid| self.delete_playlist(&uid));
        };
        let (name, query) =
            (text(&view, "name", "Playlist"), view.get("query").and_then(Value::as_str).map(String::from));
        let current = uid.as_ref().and_then(|uid| self.store.read(|lib| lib.playlist(uid)).ok());
        let (rules, held) = playlist_rules(&view, &current.map(|p| p.rules).unwrap_or_default());
        hold(kept, key, held);
        let uid = match uid {
            Some(uid) => uid,
            None => self
                .store
                .write(|lib| renamed(&name, |name| lib.create_playlist(name, query.as_deref(), &rules)))?,
        };
        links.insert(key.to_owned(), uid.clone());
        let mut order: Vec<(f64, &String, &str)> = view
            .iter()
            .filter(|(k, _)| k.starts_with("e:"))
            .filter_map(|(id, e)| Some((e["p"].as_f64()?, id, e["i"].as_str()?)))
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(b.1)));
        let wanted: Vec<&str> = order
            .iter()
            .filter_map(|(.., item)| local.items.get(*item)?.first().map(String::as_str))
            .collect();
        let mut shown = name.clone();
        self.edit_playlist(&uid, |lib, info| {
            let updated = PlaylistInfo { name: name.clone(), query: query.clone(), rules, ..info.clone() };
            if updated != info && lib.update_playlist(&updated).is_err() {
                shown.clone_from(&info.name);
                lib.update_playlist(&PlaylistInfo { name: info.name.clone(), ..updated })?;
            }
            let current: Vec<String> = lib.playlist_items(&uid, now())?.into_iter().map(|i| i.uid).collect();
            if query.is_none() && current.iter().map(String::as_str).ne(wanted.iter().copied()) {
                lib.remove_from_playlist_many(&uid, &current.iter().map(String::as_str).collect::<Vec<_>>())?;
                lib.add_to_playlist(&uid, &wanted)?;
            }
            Ok(Vec::new())
        })?;
        keep_name(kept, key, &shown, &name);
        Ok(())
    }

    pub fn copy_playlists(&self, local: &Local, copies: Vec<(String, View)>, links: &mut Links, ids: &Ids) {
        for (_, mut view) in copies.into_iter().filter(|(key, _)| key.starts_with("playlists/")) {
            let name = crate::words::conflict(&text(&view, "name", "Playlist"));
            view.insert("name".into(), json!(name));
            let draft = "playlists/copy".to_owned();
            let made = self.apply_playlist(local, &draft, Some(view), links, &mut Kept::new());
            match (made, links.remove(&draft)) {
                (Ok(()), Some(uid)) => {
                    links.insert(format!("playlists/{}", ids.named("playlist", &uid)), uid);
                    tracing::info!(
                        "two devices changed a playlist in ways that do not merge; both were kept"
                    );
                }
                (made, _) => {
                    tracing::warn!("the conflict copy of a playlist could not be made: {:?}", made.err())
                }
            }
        }
    }
}
