use super::local::{DEVICE_KEYS, Local};
use crate::daemon::Daemon;
use crate::error::{Error, Result};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_sync::View;

type Links = BTreeMap<String, String>;

fn rank(key: &str) -> u8 {
    match key.split_once('/').map_or(key, |(_, item)| item).split('_').next() {
        _ if key.starts_with("library/") => 0,
        Some("preset") => 1,
        _ if key.starts_with("playlists/") => 2,
        Some("profile") => 3,
        _ => 4,
    }
}

impl Daemon {
    pub fn apply_remote(
        &self,
        local: &Local,
        mut applied: Vec<(String, Option<View>)>,
        links: &mut Links,
    ) -> u32 {
        applied.sort_by_key(|(key, _)| rank(key));
        let mut done = 0;
        for (key, view) in applied {
            let (collection, item) = key.split_once('/').unwrap_or_default();
            let result = match (collection, item.split_once('_').map(|(kind, _)| kind)) {
                ("library", _) => view.map_or(Ok(()), |view| self.apply_item(local, item, &view)),
                ("playlists", _) => self.apply_playlist(local, &key, view, links),
                ("settings", Some("preset")) => self.apply_preset(local, &key, view, links),
                ("settings", Some("profile")) => self.apply_profile(local, &key, view, links),
                ("settings", _) => view.map_or(Ok(()), |view| self.apply_settings(&view)),
                _ => Ok(()),
            };
            match result {
                Ok(()) => done += 1,
                Err(e) => {
                    tracing::warn!(collection, "a synced change could not be applied");
                    tracing::debug!("{e}");
                }
            }
        }
        if done > 0 {
            self.library_changed(Vec::new());
            ["playlists", "profiles"].into_iter().for_each(|topic| self.changes.changed(topic, Vec::new()));
        }
        done
    }

    fn apply_item(&self, local: &Local, key: &str, view: &View) -> Result<()> {
        let tags: Vec<&str> = view.keys().filter_map(|key| key.strip_prefix("t:")).collect();
        for uid in local.items.get(key).into_iter().flatten() {
            self.store.write(|lib| {
                let item = lib.item(uid)?;
                let rating = view.get("rating").and_then(Value::as_u64).unwrap_or(0).min(5) as u8;
                if item.rating != rating {
                    lib.set_rating(uid, rating)?;
                }
                let favorite = view.get("favorite") == Some(&json!(true));
                if item.favorite != favorite {
                    lib.set_favorite(uid, favorite)?;
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
                Ok::<_, Error>(())
            })?;
        }
        Ok(())
    }

    fn apply_settings(&self, view: &View) -> Result<()> {
        let options = self.state().config.options();
        for (key, value) in view.iter().filter(|(key, _)| !DEVICE_KEYS.contains(&key.as_str())) {
            let current = options.get(key).and_then(|v| serde_json::to_value(v).ok());
            if current.is_some() && current.as_ref() != Some(value) {
                self.set_option(key, &value.as_str().map_or_else(|| value.to_string(), String::from))?;
            }
        }
        Ok(())
    }
}
