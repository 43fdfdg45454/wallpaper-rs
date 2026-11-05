use super::*;
use crate::page;

fn page_with(playback: Value) -> (String, std::path::PathBuf) {
    page_named("paced", playback, json!({}))
}

fn page_named(name: &str, playback: Value, properties: Value) -> (String, std::path::PathBuf) {
    let source = store::dir().unwrap().join(format!("paced-{name}/{name}"));
    fs::create_dir_all(&source).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": format!("org.example.{name}"), "version": "1.0.0",
        "title": "Paced", "license": "CC0-1.0", "kind": "web", "entry": "index.html", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" }, "playback": playback, "properties": properties });
    fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
    fs::write(source.join("index.html"), "<!DOCTYPE html>\n<html><body></body></html>").unwrap();
    let uids = ask(json!({ "op": "examples", "dir": source.parent().unwrap() }));
    let uid = uids[0].as_str().unwrap_or_else(|| panic!("{uids}")).to_owned();
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    (uid, path)
}

pub(super) fn pages_fade_out_while_another_app_plays() {
    let pack = pack::load(&page_with(json!({ "volume": 1.0 })).1).unwrap();
    let volume = || page::settings(&pack, false, page::privacy(&pack))["volume"].as_f64().unwrap();
    settings::setting("playback.muted", Some("false")).unwrap();
    assert_eq!(volume(), 1.0);
    page::duck(0.25);
    assert_eq!(volume(), 0.25);
    for wild in [-1.0, 7.0, f32::NAN] {
        page::duck(wild);
        assert!((0.0..=1.0).contains(&volume()), "{wild} gave {}", volume());
    }
    page::duck(1.0);
    settings::setting("playback.muted", Some("true")).unwrap();
    assert_eq!(volume(), 0.0);
    assert_eq!(settings::setting("playback.duck", Some("true")).unwrap(), "true");
    assert!(settings::setting("playback.duck", Some("loud")).is_err());
    assert_eq!(settings::setting("playback.duck", Some("false")).unwrap(), "false");
}

pub(super) fn pages_keep_the_frame_cap_and_the_volume_of_their_author() {
    settings::setting("playback.muted", Some("false")).unwrap();
    let (uid, path) = page_named("quiet", json!({ "max_fps": 12, "muted": true }), json!({}));
    let pack = pack::load(&path).unwrap();
    let shown = || page::settings(&pack, false, page::privacy(&pack));
    assert_eq!((shown()["fps"].clone(), shown()["volume"].clone()), (json!(12), json!(0.0)));
    let form = ask(json!({ "op": "scene", "uid": uid, "languages": [] }));
    let volume =
        form["fields"].as_array().unwrap().iter().find(|f| f["key"] == json!("volume")).cloned().unwrap();
    assert_eq!(
        (volume["value"].clone(), form["web"]["own_volume"].clone()),
        (json!(0), json!(false)),
        "{form}"
    );
    assert_eq!(ask(json!({ "op": "property", "uid": uid, "key": "volume", "value": 50 })), Value::Null);
    assert_eq!(shown()["volume"], json!(0.5));
    assert!(
        ask(json!({ "op": "property", "uid": uid, "key": "volume", "value": "loud" }))["error"].is_string()
    );
    let own =
        json!({ "volume": { "type": "slider", "min": 0, "max": 10, "default": 3, "label": "Loudness" } });
    let (mine, path) = page_named("loud", json!({ "max_fps": "display" }), own);
    let pack = pack::load(&path).unwrap();
    let settings = page::settings(&pack, false, page::privacy(&pack));
    assert_eq!(settings["volume"], json!(1.0), "a page with a volume of its own keeps the general one");
    assert_eq!(settings["fps"], json!(crate::power::Power::now().fps(false)));
    let form = ask(json!({ "op": "scene", "uid": mine, "languages": [] }));
    assert_eq!(form["web"]["own_volume"], json!(true));
    settings::setting("playback.muted", Some("true")).unwrap();
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid, mine] })), Value::Null);
}

pub(super) fn shortcuts_open_what_the_user_chose_after_a_tap() {
    let open = json!({ "open": { "type": "shortcut", "label": "Open" } });
    let (uid, path) = page_named("opens", json!({}), open);
    let set = |value: Value| ask(json!({ "op": "property", "uid": uid, "key": "open", "value": value }));
    for bad in [
        json!({ "uri": "http://example.org" }),
        json!({ "file": "/sdcard/notes.txt" }),
        json!({ "app": "a/b" }),
        json!(1),
    ] {
        assert!(set(bad.clone())["error"].is_string(), "{bad}");
    }
    assert_eq!(set(json!({ "uri": "https://example.org/" })), Value::Null);
    let pack = pack::load(&path).unwrap();
    assert_eq!(crate::opened::chosen(&pack, "open"), Some(json!({ "uri": "https://example.org/" })));
    assert_eq!(crate::opened::chosen(&pack, "missing"), None);
    let handle = 0x0be7;
    crate::opened::pressed(handle);
    crate::opened::page(handle, &pack, "open");
    assert!(crate::opened::taken(handle).is_empty(), "the package does not hold the taps grant");
    ask(json!({ "op": "grant", "uid": uid, "input": "taps", "on": true }));
    crate::opened::page(handle, &pack, "open");
    assert_eq!(crate::opened::taken(handle), vec![json!({ "uri": "https://example.org/" })]);
    crate::opened::page(handle, &pack, "open");
    assert!(crate::opened::taken(handle).is_empty(), "one tap opens once");
    crate::opened::pressed(handle);
    crate::opened::page(handle, &pack, "open");
    assert!(crate::opened::taken(handle).is_empty(), "at most once a second");
    std::thread::sleep(std::time::Duration::from_millis(1050));
    crate::opened::pressed(handle);
    crate::opened::page(handle, &pack, "open");
    assert_eq!(crate::opened::taken(handle).len(), 1);
    assert_eq!(set(json!({ "app": "org.example.notes" })), Value::Null);
    assert_eq!(crate::opened::chosen(&pack, "open"), Some(json!({ "app": "org.example.notes" })));
    assert_eq!(set(Value::Null), Value::Null);
    assert_eq!(crate::opened::chosen(&pack, "open"), None);
    crate::opened::forget(handle);
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
