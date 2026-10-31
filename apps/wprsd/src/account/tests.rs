use super::files::opted;
use super::presets::synced_options;
use super::saved::Files;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use wprs_library::Rules;
use wprs_schedule::Order;
use wprs_sync::{
    DEVICE_KEYS, Doc, Hlc, Kept, Local, Replica, Rules as Merge, SyncKey, View, hold, keep_name,
    playlist_rules, preserve, settings, step,
};

fn view(pairs: &[(&str, serde_json::Value)]) -> View {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn doc(pairs: &[(&str, serde_json::Value)], ms: u64) -> Doc {
    Doc::stamp(None, Some(&view(pairs)), |_| false, &Hlc(ms, 0, "a".into())).unwrap()
}

#[test]
fn values_this_version_does_not_know_go_back_unchanged() {
    let mut replica = Replica::default();
    let list = [("name", json!("Sea")), ("r:order", json!("weighted")), ("color", json!("teal"))];
    replica.bases.insert("playlists/p".into(), (3, doc(&list, 1)));
    let blob = json!({"id": "f0", "hash": "aa", "size": 3, "name": "a.png"});
    replica.bases.insert("library/k".into(), (4, doc(&[("title", json!("A")), ("blob", blob.clone())], 1)));
    let local = |order: &str| view(&[("name", json!("Sea")), ("r:order", json!(order))]);
    let mut views = BTreeMap::from([("playlists/p".to_string(), local("sequential"))]);
    views.insert("library/k".into(), view(&[("title", json!("A"))]));
    let mut kept: Kept =
        BTreeMap::from([("playlists/p".into(), BTreeMap::from([("r:order".into(), json!("sequential"))]))]);
    preserve(&mut views, &replica, &mut kept, &BTreeSet::new());
    assert_eq!(views["playlists/p"], view(&list));
    assert_eq!(views["library/k"]["blob"], blob);
    assert_eq!(kept.len(), 1);
    let mut views = BTreeMap::from([("playlists/p".to_string(), local("shuffle"))]);
    views.insert("library/k".into(), view(&[("title", json!("A"))]));
    preserve(&mut views, &replica, &mut kept, &BTreeSet::from(["k".to_string()]));
    assert_eq!(views["playlists/p"]["r:order"], "shuffle");
    assert!(kept.is_empty() && !views["library/k"].contains_key("blob"));
}

#[test]
fn rules_apply_field_by_field_and_hold_unknown_values() {
    let current = Rules { order: Order::Shuffle, transition_ms: Some(300), ..Rules::default() };
    let synced =
        view(&[("r:order", json!("weighted")), ("r:interval", json!("5m")), ("r:resume", json!(false))]);
    let (rules, held) = playlist_rules(&synced, &current);
    assert_eq!((rules.order, rules.interval.as_secs(), rules.resume), (Order::Shuffle, 300, false));
    assert_eq!(rules.transition_ms, None);
    assert_eq!(held, BTreeMap::from([("r:order".to_string(), json!("shuffle"))]));
    assert!(playlist_rules(&view(&[("r:future", json!(1))]), &current).1.is_empty());
}

#[test]
fn a_change_that_fails_here_is_held_without_being_synced() {
    let old = (3, doc(&[("name", json!("Sea"))], 1));
    let new = (5, doc(&[("name", json!("Lake"))], 2));
    let mut replica = Replica::default();
    replica.bases.insert("playlists/p".into(), new.clone());
    let mut local = Local::default();
    local.views.insert("playlists/p".into(), view(&[("name", json!("Sea"))]));
    let before = BTreeMap::from([("playlists/p".to_string(), old.clone())]);
    let failed = [("playlists/p".to_string(), "the name is taken".to_string())];
    let held = hold(&mut replica, &before, &local, &failed, &Hlc(9, 0, "b".into()));
    assert_eq!(held, BTreeMap::from([("playlists/p".to_string(), 5)]));
    assert_eq!(
        (replica.bases["playlists/p"].clone(), replica.held["playlists/p"].clone()),
        (old, new.clone())
    );
    let next = step(
        replica.bases.get("playlists/p"),
        Some(&new),
        local.views.get("playlists/p"),
        Merge::default(),
        &Hlc(10, 0, "b".into()),
    );
    assert_eq!((next.push, next.apply), (None, Some(Some(view(&[("name", json!("Lake"))])))));
}

#[test]
fn a_received_name_taken_here_is_not_sent_back() {
    let mut replica = Replica::default();
    replica.bases.insert("playlists/p".into(), (3, doc(&[("name", json!("Mix"))], 2)));
    let mut kept = Kept::new();
    keep_name(&mut kept, "playlists/p", "Mix (2)", "Mix");
    let mut views = BTreeMap::from([("playlists/p".to_string(), view(&[("name", json!("Mix (2)"))]))]);
    preserve(&mut views, &replica, &mut kept, &BTreeSet::new());
    assert_eq!(views["playlists/p"]["name"], "Mix");
    let mut views = BTreeMap::from([("playlists/p".to_string(), view(&[("name", json!("Evening"))]))]);
    preserve(&mut views, &replica, &mut kept, &BTreeSet::new());
    assert_eq!((views["playlists/p"]["name"].as_str(), kept.is_empty()), (Some("Evening"), true));
    keep_name(&mut kept, "playlists/p", "Mix", "Mix");
    assert!(kept.is_empty());
}

#[test]
fn device_settings_stay_out_of_synced_settings_and_profiles() {
    let (ids, options) = (SyncKey::generate().ids(), wprs_core::Config::default().options());
    let sections = settings(&options, &Replica::default(), &ids, &DEVICE_KEYS);
    let synced: BTreeSet<&str> = sections.values().flat_map(|view| view.keys().map(String::as_str)).collect();
    assert!(synced.contains("playback.max_fps") && synced.len() + DEVICE_KEYS.len() == options.len());
    assert!(DEVICE_KEYS.iter().all(|key| options.contains_key(*key) && !synced.contains(key)));
    assert!(!sections.contains_key(&format!("settings/{}", ids.named("section", "models"))));
    let (volume, parallax, battery) = ("playback.volume", "playback.software_parallax", "pause.on_battery");
    let profile = json!({volume: "0.2", "models.allow_download": "false", battery: "false"});
    let here = json!({volume: "0.9", parallax: "true", battery: "true"});
    assert_eq!(synced_options(&profile, None), json!({battery: "false"}));
    let kept = json!({battery: "false", volume: "0.9", parallax: "true"});
    assert_eq!(synced_options(&profile, Some(&here)), kept);
    assert_eq!(synced_options(&json!(null), Some(&here)), json!(null));
}

#[test]
fn shared_folders_leave_depth_maps_out() {
    let files = Files { folders: BTreeSet::from(["/pictures".into()]), ..Files::default() };
    let shared = |name: &str| opted(&files, "item", &Path::new("/pictures").join(name));
    let names = ["hills.jpg", "hills.jpg.depth.png", "x.PNG.DEPTH.PNG"];
    assert_eq!(names.map(shared), [true, false, false]);
}

#[test]
fn waiting_changes_are_tried_again_later_and_later() {
    let waits: Vec<u64> = (1..=8).map(|tries| super::watch::backoff(tries).as_secs()).collect();
    assert_eq!(waits, [30, 60, 120, 240, 480, 900, 900, 900]);
}
