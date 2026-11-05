use super::*;
use crate::page;

fn page_with(playback: Value) -> std::path::PathBuf {
    let source = store::dir().unwrap().join("paced-pages/paced");
    fs::create_dir_all(&source).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.paced", "version": "1.0.0",
        "title": "Paced", "license": "CC0-1.0", "kind": "web", "entry": "index.html", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" }, "playback": playback });
    fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
    fs::write(source.join("index.html"), "<!DOCTYPE html>\n<html><body></body></html>").unwrap();
    let uids = ask(json!({ "op": "examples", "dir": source.parent().unwrap() }));
    let uid = uids[0].as_str().unwrap_or_else(|| panic!("{uids}")).to_owned();
    library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap()
}

pub(super) fn pages_fade_out_while_another_app_plays() {
    let pack = pack::load(&page_with(json!({ "volume": 1.0 }))).unwrap();
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
