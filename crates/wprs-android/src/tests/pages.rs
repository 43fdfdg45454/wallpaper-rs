use super::{FIXTURES, ask};
use crate::page::{self, script, serve::serve};
use crate::{library, pack, props, store};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

const HANDLE: i64 = 0x5eed;

const MANIFEST: &str = r##"{
  "schema": "wallpaper-rs.wpak/2", "id": "org.example.phone", "version": "1.0.0", "title": "Phone", "default_locale": "en",
  "license": "CC0-1.0", "origin": { "kind": "original" }, "content": { "rating": "general" },
  "photosensitive": false, "kind": "web", "entry": "index.html", "inputs": ["audio", "pointer", "network"],
  "permissions": { "network": { "allow": ["https://api.example.org"] } },
  "web": { "background": "#102030", "storage_kib": 64, "preload": "early.js" },
  "properties": {
    "tint": { "type": "color", "default": "#ff8000", "label": "Tint" },
    "mood": { "type": "combo", "default": "calm", "label": "Mood",
              "options": [{ "value": "calm", "label": { "en": "Calm", "es": "Calma" } }] },
    "photos": { "type": "folder", "mode": "all", "accept": ["image"], "label": "Photos" }
  }
}"##;

fn installed() -> (String, std::path::PathBuf) {
    let source = store::dir().unwrap().join("phone-pages/phone");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("manifest.json"), MANIFEST).unwrap();
    fs::write(
        source.join("index.html"),
        "<!DOCTYPE html>\n<html><body><script src=\"a b.js\"></script></body></html>",
    )
    .unwrap();
    fs::write(source.join("a b.js"), "window.ran = true;").unwrap();
    fs::write(source.join("early.js"), "window.early = 1;").unwrap();
    let uids = ask(json!({ "op": "examples", "dir": source.parent().unwrap() }));
    let uid = uids[0].as_str().unwrap_or_else(|| panic!("{uids}")).to_owned();
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    (uid, path)
}

fn served(url: &str) -> Value {
    serve(HANDLE, url).unwrap()
}

pub(super) fn pages_are_served_from_their_package_only() {
    let (uid, path) = installed();
    let item = ask(json!({ "op": "item", "uid": uid }));
    let found = ask(json!({ "op": "items", "query": "kind:web" }));
    assert!(found.as_array().unwrap().iter().any(|i| i["uid"] == json!(uid)), "{found}");
    assert_eq!(item["package"]["exposes"], json!(["zone", "language", "gpu", "storage"]), "{item}");
    let spec = page::spec(HANDLE, &uid, &path, false).unwrap();
    let url = spec["url"].as_str().unwrap().to_owned();
    assert!(url.starts_with("https://") && url.ends_with(".wpak.invalid/") && !url.contains("example"));
    assert_eq!((spec["background"].clone(), spec["network"].clone()), (json!("#102030"), json!(false)));
    let index = served(&url);
    let html = fs::read_to_string(index["path"].as_str().unwrap()).unwrap();
    assert!(html.starts_with("<!DOCTYPE html><script src=\"/:wprs/api.js\"></script>\n<html>"), "{html}");
    let csp = index["csp"].as_str().unwrap();
    assert!(csp.contains("connect-src 'self' data: blob:;") && csp.contains("frame-src 'self'"), "{csp}");
    assert!(served(&format!("{url}a%20b.js"))["path"].is_string());
    for blocked in ["manifest.json", "../manifest.json", "missing.js", ":user/photos/0000000000000000.png"] {
        assert_eq!(served(&format!("{url}{blocked}")), json!({}), "{blocked}");
    }
    for blocked in [
        "https://api.example.org/x",
        "http://127.0.0.1:9/",
        "file:///etc/hosts",
        "wpak://org.example.phone/index.html",
    ] {
        assert_eq!(served(blocked), json!({}), "{blocked}");
    }
    let api = served(&format!("{url}:wprs/api.js"))["text"].as_str().unwrap().to_owned();
    assert!(
        api.contains("window.wprsHost")
            && api.contains("\"tint\":\"#ff8000\"")
            && api.ends_with("window.early = 1;")
    );
    assert!(api.contains("})(false, false, null, \"https://") && !api.contains("WEBGL_debug_renderer_info"));
    values_reach_the_running_page(&uid, &path, &url);
    privacy_is_chosen_per_package(&uid, &path, &url);
    page::close(HANDLE);
    assert_eq!(served(&url), json!({}));
}

fn values_reach_the_running_page(uid: &str, path: &Path, url: &str) {
    assert!(page::spec(HANDLE, uid, path, false).unwrap()["tell"].is_null());
    ask(json!({ "op": "property", "uid": uid, "key": "tint", "value": "#00ff00" }));
    let photos = store::dir().unwrap().join("phone-photos");
    fs::create_dir_all(&photos).unwrap();
    fs::copy(Path::new(FIXTURES).join("green-64x32.png"), photos.join("a.png")).unwrap();
    let pack = pack::load(path).unwrap();
    props::change(&pack, |v| {
        drop(v.insert("photos".into(), toml::Value::String(photos.display().to_string())))
    })
    .unwrap();
    let tell = page::spec(HANDLE, uid, path, false).unwrap()["tell"].as_str().unwrap().to_owned();
    assert!(
        tell.contains("\"tint\":\"#00ff00\"") && tell.contains(&format!("\"photos\":\"{url}:user/photos/\"")),
        "{tell}"
    );
    assert!(tell.contains("\"added\":[\"") && !tell.contains(photos.to_str().unwrap()), "{tell}");
    let listed = script::heard(HANDLE, r#"{"files":"photos","id":3}"#).unwrap();
    let token = listed.split('"').find(|t| t.starts_with(&format!("{url}:user/photos/"))).unwrap().to_owned();
    assert!(token.ends_with(".png"), "{listed}");
    assert_eq!(served(&token)["path"], json!(photos.join("a.png")));
    assert!(script::heard(HANDLE, r#"{"random":"photos","id":4}"#).unwrap().contains(&token));
    fs::copy(Path::new(FIXTURES).join("green-64x32.png"), photos.join("b.png")).unwrap();
    fs::remove_file(photos.join("a.png")).unwrap();
    let tell = page::spec(HANDLE, uid, path, false).unwrap()["tell"].as_str().unwrap().to_owned();
    assert!(tell.contains(&format!("\"removed\":[\"{token}\"]")), "{tell}");
    assert!(page::spec(HANDLE, uid, path, false).unwrap()["tell"].is_null());
    assert!(!page::hearing(HANDLE) && script::heard(HANDLE, r#"{"audio":true}"#).is_none());
    ask(json!({ "op": "grant", "uid": uid, "input": "audio", "on": true }));
    assert!(page::hearing(HANDLE) && script::listen(HANDLE).unwrap().contains("\"audio\":[["));
}

fn privacy_is_chosen_per_package(uid: &str, path: &Path, url: &str) {
    let key = page::spec(HANDLE, uid, path, false).unwrap()["key"].clone();
    for what in ["hide_zone", "hide_gpu"] {
        ask(json!({ "op": "confirm", "uid": uid, "what": what }));
    }
    let spec = page::spec(HANDLE, uid, path, false).unwrap();
    assert_ne!(spec["key"], key);
    let api = served(&format!("{url}:wprs/api.js"))["text"].as_str().unwrap().to_owned();
    assert!(api.contains("WEBGL_debug_renderer_info") && api.contains("})(true, false, (g) => {"), "{api}");
    let worker = served(&format!("{url}:wprs/worker.js?classic&{}", "data%3Atext%2Fjavascript%2C1"));
    let worker = worker["text"].as_str().unwrap_or_default().to_owned();
    assert!(
        worker.contains("})(true, false, (g) => {")
            && worker.ends_with("importScripts(\"data:text/javascript,1\");")
    );
    assert_eq!(served(&format!("{url}:wprs/worker.js?classic&https%3A%2F%2Fexample.org%2Fw.js")), json!({}));
    let form = ask(json!({ "op": "scene", "uid": uid, "languages": ["en"] }));
    assert_eq!(form["web"]["exposes"], json!(["language", "storage"]), "{form}");
    assert_eq!((form["uses"]["audio"].clone(), form["web"]["network"].clone()), (json!(true), json!(true)));
    language_is_hidden_on_request(uid, path, url);
    ask(json!({ "op": "grant", "uid": uid, "input": "network", "on": true }));
    let spec = page::spec(HANDLE, uid, path, false).unwrap();
    assert_eq!(spec["network"], json!(true));
    assert_eq!(served("https://api.example.org/x"), json!({ "network": true }));
    assert!(
        served(url)["csp"]
            .as_str()
            .unwrap()
            .contains("connect-src 'self' data: blob: https://api.example.org;")
    );
    assert_eq!(ask(json!({ "op": "clear_data", "uid": uid })), Value::Null);
    let origin = spec["origin"].clone();
    assert_eq!(ask(json!({ "op": "cleared" })), json!([origin]));
    assert_eq!(ask(json!({ "op": "cleared" })), json!([]));
    let fresh = page::spec(HANDLE, uid, path, false).unwrap();
    assert!(fresh["key"] != spec["key"] && fresh["origin"] != origin, "{fresh}");
    ask(json!({ "op": "confirm", "uid": uid, "what": "hide_storage" }));
    let passing = page::spec(HANDLE, uid, path, false).unwrap()["origin"].clone();
    assert!(
        passing.as_str().unwrap().ends_with(".e.wpak.invalid") && passing != fresh["origin"],
        "{passing}"
    );
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
    assert_eq!(ask(json!({ "op": "cleared" })), json!([fresh["origin"]]));
}

fn language_is_hidden_on_request(uid: &str, path: &Path, url: &str) {
    ask(json!({ "op": "scene", "uid": uid, "languages": ["es"] }));
    let api = || {
        page::spec(HANDLE, uid, path, false).unwrap();
        served(&format!("{url}:wprs/api.js"))["text"].as_str().unwrap().to_owned()
    };
    let shown = api();
    assert!(shown.contains("\"locale\":\"es") && shown.contains("\"Calma\""), "{shown}");
    ask(json!({ "op": "confirm", "uid": uid, "what": "hide_language" }));
    let hidden = api();
    assert!(hidden.contains("\"locale\":\"en-US\"") && hidden.contains("\"Calm\""), "{hidden}");
    assert!(!hidden.contains("Calma") && hidden.contains("})(true, true, (g) => {"), "{hidden}");
}
