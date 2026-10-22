use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::Result;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use wprs_sync::{Clock, Doc, Hlc, Ids, Outcome, Remote, Replica, Session};

type Links = BTreeMap<String, String>;
type Kept = (Replica, Links, Clock, BTreeSet<String>, BTreeMap<String, i64>);

pub fn kind(doc: &Doc) -> &str {
    doc.fields.get("kind").and_then(|(value, _)| value.as_str()).unwrap_or("section")
}

pub fn object(key: &str, doc: Option<&Doc>) -> bool {
    key.starts_with("playlists/") || doc.is_some_and(|doc| kind(doc) != "section")
}

fn legacy(key: &str) -> Option<&str> {
    let item = key.strip_prefix("settings/")?;
    let keyed = item.len() == 32 && item.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    (!keyed).then_some(item)
}

fn renamed(item: &str, ids: &Ids) -> (String, Option<&'static str>) {
    match item.split_once('_') {
        Some((kind @ ("profile" | "preset"), id)) if id.len() == 32 => {
            (format!("settings/{id}"), Some(if kind == "profile" { "profile" } else { "preset" }))
        }
        _ => (format!("settings/{}", ids.named("section", item)), None),
    }
}

impl Daemon {
    pub fn migrated(&self, ids: &Ids) -> Kept {
        let names = ids.named("names", "");
        let mut s = self.saved();
        if !s.names.is_empty() && s.names != names {
            (s.replica, s.links, s.present, s.quarantine, s.legacy) = Default::default();
        }
        s.names = names;
        (s.replica.clone(), s.links.clone(), s.clock.clone(), s.present.clone(), s.legacy.clone())
    }

    pub fn migrate(
        &self,
        session: &Session,
        ids: &Ids,
        replica: &mut Replica,
        links: &mut Links,
        old: &mut BTreeMap<String, i64>,
    ) -> Result<()> {
        let found: Vec<String> = replica.bases.keys().filter(|k| legacy(k).is_some()).cloned().collect();
        for key in found {
            let Some((seq, mut doc)) = replica.bases.remove(&key) else { continue };
            let (new, kind) = renamed(legacy(&key).unwrap_or_default(), ids);
            if let Some(uid) = links.remove(&key) {
                links.insert(new.clone(), uid);
            }
            if doc == Doc::cleared() {
                continue;
            }
            if let Some(kind) = kind {
                doc.fields.insert("kind".into(), (json!(kind), Hlc::default()));
            }
            old.insert(key, seq);
            if replica.bases.contains_key(&new) {
                continue;
            }
            match block_on(session.push(&[(new.clone(), 0, doc.clone())])).map_err(sync_error)?.pop() {
                Some(Outcome::Ok(seq)) => drop(replica.bases.insert(new, (seq, doc))),
                Some(Outcome::Conflict(Some(current))) => drop(replica.bases.insert(new, current)),
                _ => {}
            }
        }
        if old.is_empty() {
            return Ok(());
        }
        let gone: Vec<_> = old
            .iter()
            .map(|(key, seq)| {
                let (collection, item) = key.split_once('/').unwrap_or_default();
                json!({"collection": collection, "item": item, "base": seq, "deleted": true})
            })
            .collect();
        let keys: Vec<String> = old.keys().cloned().collect();
        for (chunk, keys) in gone.chunks(500).zip(keys.chunks(500)) {
            let body = json!({"epoch": session.epoch, "changes": chunk});
            let answer = block_on(session.api.post("/api/v1/sync/changes", body)).map_err(sync_error)?;
            let results = answer["results"].as_array().into_iter().flatten();
            for (key, result) in keys.iter().zip(results) {
                if ["ok", "conflict"].contains(&result["status"].as_str().unwrap_or_default()) {
                    old.remove(key);
                }
            }
        }
        tracing::info!("synced settings, profiles and presets now use keyed names");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wprs_sync::SyncKey;

    #[test]
    fn plain_setting_names_become_keyed_names_that_hide_their_kind() {
        let ids = SyncKey::generate().ids();
        let profile = format!("profile_{}", "a1".repeat(16));
        assert_eq!(legacy(&format!("settings/{profile}")), Some(profile.as_str()));
        assert_eq!(legacy("settings/playback"), Some("playback"));
        assert_eq!(legacy(&format!("settings/{}", "b2".repeat(16))), None);
        assert_eq!(legacy("playlists/x"), None);
        assert_eq!(renamed(&profile, &ids), (format!("settings/{}", "a1".repeat(16)), Some("profile")));
        let (section, named) = renamed("playback", &ids);
        assert_eq!((section.len(), named), ("settings/".len() + 32, None));
        assert_eq!(legacy(&section), None);
        let mut doc = Doc::default();
        assert!(!object("settings/x", Some(&doc)) && object("playlists/x", None));
        doc.fields.insert("kind".into(), (json!("preset"), Hlc::default()));
        assert!(object("settings/x", Some(&doc)) && kind(&doc) == "preset");
    }
}
