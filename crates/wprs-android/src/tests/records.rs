use super::open;
use crate::{account, library, local, settings, store, vault};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use wprs_sync::{Doc, Hlc, Kept, Replica, Secrets, SyncKey, View};

fn view(value: Value) -> View {
    serde_json::from_value(value).unwrap()
}

pub(super) fn records() {
    let ask =
        |request: Value| serde_json::from_str::<Value>(&account::request(&request.to_string())).unwrap();
    assert_eq!(
        (
            ask(json!({ "op": "status" }))["state"].as_str(),
            ask(json!({ "op": "status" }))["active"].as_bool()
        ),
        (Some("none"), Some(false))
    );
    assert!(
        ask(json!({ "op": "pause", "paused": true }))["error"].as_str().unwrap().contains("add a server")
    );
    assert!(
        ask(json!({ "op": "add", "url": "http://example.org" }))["error"].as_str().unwrap().contains("https")
    );
    let mut secrets = Secrets::default();
    secrets.token = "t".into();
    vault::store(&secrets).unwrap();
    assert_eq!(vault::load().unwrap().token, "t");
    vault::forget().unwrap();
    assert!(vault::load().is_err());

    let ids = SyncKey::generate().ids();
    let doc = |value: Value| Doc::stamp(None, Some(&view(value)), |_| false, &Hlc(1, 0, "a".into())).unwrap();
    let section = |name: &str| format!("settings/{}", ids.named("section", name));
    let mut replica = Replica::default();
    replica.bases.insert(
        section("playback"),
        (1, doc(json!({ "playback.max_fps": "display", "playback.volume": 0.2 }))),
    );
    replica.bases.insert("settings/p".into(), (2, doc(json!({ "kind": "profile", "name": "Calm" }))));
    replica.bases.insert("repos/r".into(), (3, doc(json!({ "url": "https://example.org/repo" }))));
    let (mut links, mut kept) = (BTreeMap::new(), Kept::new());
    let (green, _) = store::store(&mut open("green-64x32.png"), "green.png").unwrap();
    let local = local::snapshot(&ids, &replica, &mut links, &BTreeSet::new(), &mut kept).unwrap();
    let echo = |key: &str| replica.bases[key].1.view();
    assert_eq!(
        (local.views.get("settings/p").cloned(), local.views.get("repos/r").cloned()),
        (echo("settings/p"), echo("repos/r"))
    );
    let playback = &local.views[&section("playback")];
    assert_eq!((&playback["playback.max_fps"], playback.get("playback.muted")), (&json!("display"), None));
    let key = local.keys[&green].clone();
    let list = json!({ "name": "Shared", "r:order": "shuffle", "e:a-0": { "i": key, "p": 1.0 }, "e:b-0": { "i": "gone", "p": 2.0 } });
    let applied = vec![
        (
            section("transition"),
            Some(view(json!({ "transition.duration_ms": 900, "playback.max_fps": "display" }))),
        ),
        (format!("library/{key}"), Some(view(json!({ "title": "Desk", "favorite": true, "t:sky": true })))),
        ("playlists/l".into(), Some(view(list))),
        ("settings/p".into(), Some(view(json!({ "kind": "profile", "name": "Calmer" })))),
    ];
    assert_eq!(local::apply(&local, applied, &mut links, &replica, &mut kept), (3, Vec::new()));
    let values =
        ["transition.duration_ms", "playback.max_fps"].map(|key| settings::setting(key, None).unwrap());
    assert_eq!(values, ["900", "30"]);
    let item = library::with(|lib| Ok(lib.item(&green)?)).unwrap();
    assert_eq!((item.title.as_str(), item.favorite, item.tags), ("Desk", true, vec!["sky".to_string()]));
    let shared = links["playlists/l"].clone();
    let entries = library::with(|lib| Ok(lib.playlist_items(&shared, library::now())?)).unwrap();
    assert_eq!(entries.iter().map(|i| i.uid.as_str()).collect::<Vec<_>>(), [green.as_str()]);
    assert_eq!(
        local::apply(&local, vec![("playlists/l".into(), None)], &mut links, &replica, &mut kept).0,
        1
    );
    assert!(library::with(|lib| Ok(lib.playlist(&shared).is_err())).unwrap() && links.is_empty());
}
