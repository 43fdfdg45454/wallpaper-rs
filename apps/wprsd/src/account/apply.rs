use super::legacy::kind;
use super::local::{DEVICE_KEYS, Local};
use super::views::{Kept, item_view};
use crate::daemon::Daemon;
use crate::error::{Error, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_sync::{Replica, View};

type Links = BTreeMap<String, String>;

fn rank(key: &str, kind: &str) -> u8 {
    match kind {
        _ if key.starts_with("library/") => 0,
        "preset" => 1,
        _ if key.starts_with("playlists/") => 2,
        "profile" => 3,
        _ => 4,
    }
}

pub fn hold(kept: &mut Kept, key: &str, held: BTreeMap<String, Value>) {
    match held.is_empty() {
        true => drop(kept.remove(key)),
        false => drop(kept.insert(key.to_owned(), held)),
    }
}

impl Daemon {
    pub fn apply_remote(
        self: &Arc<Self>,
        local: &Local,
        applied: Vec<(String, Option<View>)>,
        links: &mut Links,
        replica: &Replica,
        kept: &mut Kept,
    ) -> Vec<(String, String)> {
        let mut applied: Vec<(String, Option<View>, String)> = applied
            .into_iter()
            .map(|(key, view)| {
                let kind = replica.bases.get(&key).map_or("section", |(_, doc)| kind(doc)).to_owned();
                (key, view, kind)
            })
            .collect();
        applied.sort_by_key(|(key, _, kind)| rank(key, kind));
        let (mut done, mut failed) = (0, Vec::new());
        for (key, view, kind) in applied {
            let (collection, item) = key.split_once('/').unwrap_or_default();
            let result = match (collection, kind.as_str()) {
                ("library", _) => view.map_or(Ok(()), |view| self.apply_item(local, &key, item, &view, kept)),
                ("playlists", _) => self.apply_playlist(local, &key, view, links, kept),
                ("settings", "preset") => self.apply_preset(local, &key, view, links, kept),
                ("settings", "profile") => self.apply_profile(local, &key, view, links, kept),
                ("settings", _) => view.map_or(Ok(()), |view| self.apply_settings(&key, &view, kept)),
                ("repos", _) => self.apply_repo(local, &key, view),
                ("secrets", _) => self.apply_secret(view),
                _ => Ok(()),
            };
            match result {
                Ok(()) => done += 1,
                Err(e) => {
                    tracing::debug!(collection, "a synced change could not be applied: {e}");
                    failed.push((key, e.to_string()));
                }
            }
        }
        if done > 0 {
            self.library_changed(Vec::new());
            ["playlists", "profiles"].into_iter().for_each(|topic| self.changes.changed(topic, Vec::new()));
        }
        failed
    }

    fn apply_item(&self, local: &Local, key: &str, item: &str, view: &View, kept: &mut Kept) -> Result<()> {
        let tags: Vec<&str> = view.keys().filter_map(|key| key.strip_prefix("t:")).collect();
        let rating = view.get("rating").map_or(Some(0), |r| r.as_u64().filter(|r| *r <= 5));
        let favorite = view.get("favorite").map_or(Some(false), Value::as_bool);
        for uid in local.items.get(item).into_iter().flatten() {
            let held = self.store.write(|lib| {
                let item = lib.item(uid)?;
                let (shown, mut held) = (item_view(&item), BTreeMap::new());
                let mut keep = |field: &str| {
                    drop(held.insert(field.to_owned(), shown.get(field).cloned().unwrap_or_default()))
                };
                match rating {
                    Some(rating) if item.rating != rating as u8 => lib.set_rating(uid, rating as u8)?,
                    Some(_) => {}
                    None => keep("rating"),
                }
                match favorite {
                    Some(favorite) if item.favorite != favorite => lib.set_favorite(uid, favorite)?,
                    Some(_) => {}
                    None => keep("favorite"),
                }
                if let Some(title) = view.get("title").and_then(Value::as_str).filter(|t| *t != item.title) {
                    lib.set_title(uid, title)?;
                }
                let add: Vec<&str> =
                    tags.iter().copied().filter(|t| !item.tags.iter().any(|have| have == t)).collect();
                let remove: Vec<&str> =
                    item.tags.iter().map(String::as_str).filter(|t| !tags.contains(t)).collect();
                if !add.is_empty() || !remove.is_empty() {
                    lib.retag(&[uid], &add, &remove)?;
                }
                Ok::<_, Error>(held)
            })?;
            hold(kept, key, held);
        }
        Ok(())
    }

    fn apply_settings(&self, key: &str, view: &View, kept: &mut Kept) -> Result<()> {
        let (options, mut held) = (self.state().config.options(), BTreeMap::new());
        for (option, value) in view.iter().filter(|(key, _)| !DEVICE_KEYS.contains(&key.as_str())) {
            let current = options.get(option).and_then(|v| serde_json::to_value(v).ok());
            let Some(current) = current.filter(|current| current != value) else { continue };
            let text = value.as_str().map_or_else(|| value.to_string(), String::from);
            if self.set_option(option, &text).is_err() {
                tracing::info!(option, "a synced value is not known to this version; it is kept as received");
                held.insert(option.clone(), current);
            }
        }
        hold(kept, key, held);
        Ok(())
    }
}
