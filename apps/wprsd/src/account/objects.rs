use super::views::Local;
use crate::daemon::Daemon;
use crate::error::{Error, Result};
use crate::jobs::now;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use wprs_library::{Library, PlaylistInfo, Rules};
use wprs_sync::View;

pub type Links = BTreeMap<String, String>;

pub fn text(view: &View, field: &str, fallback: &str) -> String {
    view.get(field).and_then(Value::as_str).unwrap_or(fallback).to_owned()
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
    ) -> Result<()> {
        let uid = self.linked_uid(links, key, |lib, uid| lib.playlist(uid).is_ok());
        let Some(view) = view else {
            links.remove(key);
            return uid.map_or(Ok(()), |uid| self.delete_playlist(&uid));
        };
        let (name, query) =
            (text(&view, "name", "Playlist"), view.get("query").and_then(Value::as_str).map(String::from));
        let known = serde_json::to_value(Rules::default()).unwrap_or_default();
        let rules = view.iter().filter_map(|(k, v)| Some((k.strip_prefix("r:")?.to_owned(), v.clone())));
        let rules: Map<String, Value> = rules.filter(|(k, _)| known.get(k).is_some()).collect();
        let rules: Rules = serde_json::from_value(Value::Object(rules)).unwrap_or_default();
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
        self.edit_playlist(&uid, |lib, info| {
            let updated = PlaylistInfo { name: name.clone(), query: query.clone(), rules, ..info.clone() };
            if updated != info && lib.update_playlist(&updated).is_err() {
                lib.update_playlist(&PlaylistInfo { name: info.name.clone(), ..updated })?;
            }
            let current: Vec<String> = lib.playlist_items(&uid, now())?.into_iter().map(|i| i.uid).collect();
            if query.is_none() && current.iter().map(String::as_str).ne(wanted.iter().copied()) {
                lib.remove_from_playlist_many(&uid, &current.iter().map(String::as_str).collect::<Vec<_>>())?;
                lib.add_to_playlist(&uid, &wanted)?;
            }
            Ok(Vec::new())
        })
    }
}
