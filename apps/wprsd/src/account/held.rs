use super::views::Local;
use std::collections::BTreeMap;
use wprs_sync::{Doc, Hlc, Replica, Rules, View};

pub fn rules(collection: &str) -> Rules {
    match collection {
        "library" => {
            Rules { weak: |key| key == "title", keep_absent: true, needs_strong: true, ..Rules::default() }
        }
        "settings" => Rules { weak: |key| key.contains('.'), ..Rules::default() },
        "playlists" => Rules { clash: wprs_sync::clash, ..Rules::default() },
        "repos" => Rules::default(),
        _ => Rules { keep_absent: true, ..Rules::default() },
    }
}

pub fn arrived(
    local: &Local,
    replica: &Replica,
    applied: &[(String, Option<View>)],
    start: &BTreeMap<String, (i64, Doc)>,
) -> Vec<(String, Option<View>)> {
    let secret = |key: &str| {
        key.starts_with("secrets/") && start.get(key).map(|b| b.0) != replica.bases.get(key).map(|b| b.0)
    };
    let mentions =
        |view: &View| view.values().any(|v| local.fresh.iter().any(|k| v.to_string().contains(k.as_str())));
    let fresh = |key: &str| key.strip_prefix("library/").is_some_and(|k| local.fresh.contains(k));
    let views = replica.bases.iter().filter(|(key, _)| !applied.iter().any(|(k, _)| k == *key));
    let views = views.filter_map(|(key, (_, doc))| Some((key, doc.view()?)));
    let wanted =
        |key: &str, view: &View| fresh(key) || secret(key) || local.unlinked.contains(key) || mentions(view);
    views.filter(|(key, view)| wanted(key, view)).map(|(key, view)| (key.clone(), Some(view))).collect()
}

pub fn hold(
    replica: &mut Replica,
    before: &BTreeMap<String, (i64, Doc)>,
    local: &Local,
    failed: &[(String, String)],
    now: &Hlc,
) -> BTreeMap<String, i64> {
    let mut held = BTreeMap::new();
    for (key, _) in failed {
        let Some(after) = replica.bases.get(key).cloned() else { continue };
        let old = before.get(key);
        let weak = rules(key.split('/').next().unwrap_or_default()).weak;
        let shown = Doc::stamp(old.map(|(_, doc)| doc), local.views.get(key), weak, now);
        held.insert(key.clone(), after.0);
        replica.held.insert(key.clone(), after);
        match shown.or_else(|| old.map(|(_, doc)| doc.clone())) {
            Some(doc) => drop(replica.bases.insert(key.clone(), (old.map_or(0, |(seq, _)| *seq), doc))),
            None => drop(replica.bases.remove(key)),
        }
    }
    held
}
