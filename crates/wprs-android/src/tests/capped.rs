use super::ask;
use crate::page::{self, kept, script, serve::serve};
use crate::{library, store};
use rquickjs::{Context, Runtime};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

const HANDLE: i64 = 0xcab;

const FAKE: &str = r#"
class DOMException extends Error { constructor(m, name) { super(m); this.name = name; } }
class Storage {
  constructor() { this.m = new Map(); }
  get length() { return this.m.size; }
  key(i) { const all = [...this.m.keys()]; return i < all.length ? all[i] : null; }
  getItem(k) { return this.m.has(String(k)) ? this.m.get(String(k)) : null; }
  setItem(k, v) { this.m.set(String(k), String(v)); }
  removeItem(k) { this.m.delete(String(k)); }
  clear() { this.m.clear(); }
}
globalThis.window = globalThis;
globalThis.setTimeout = (run) => run();
globalThis.IDBObjectStore = class { put(v) { return `put ${v}`; } };
globalThis.localStorage = new Storage();
localStorage.setItem("old", "ab");
globalThis.posted = [];
const tried = (work) => { try { work(); return "ok"; } catch (e) { return e.name; } };
"#;

const PROBE: &str = r#"[
  posted.join(),
  tried(() => localStorage.setItem("a", "x".repeat(20))),
  tried(() => localStorage.setItem("b", "y".repeat(10))),
  String(localStorage.getItem("b")),
  tried(() => localStorage.setItem("a", "x")),
  tried(() => localStorage.setItem("b", "y".repeat(10))),
  tried(() => new Storage().setItem("z", "z".repeat(500))),
  tried(() => { localStorage.clear(); localStorage.setItem("c", "c".repeat(30)); }),
  tried(() => { localStorage.d = "dd"; }),
  tried(() => { delete localStorage.c; localStorage.e = "ee"; }),
  String(localStorage.getItem("e")),
  new IDBObjectStore().put(1),
  posted.join(),
].join(" ")"#;

#[test]
fn writes_past_the_cap_are_refused() {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    let capped = kept::capped(64, "(m) => posted.push(JSON.stringify(m))");
    let seen =
        context.with(|ctx| ctx.eval::<String, _>(format!("{FAKE}\n{capped}\n{capped}\n{PROBE}")).unwrap());
    let refused = r#"{"kept":10} ok QuotaExceededError null ok ok ok ok QuotaExceededError ok ee put 1"#;
    assert_eq!(seen, format!(r#"{refused} {{"kept":10}},{{"wrote":true}}"#));
}

fn installed() -> (String, PathBuf) {
    let source = store::dir().unwrap().join("capped-pages/capped");
    fs::create_dir_all(&source).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.capped", "version": "1.0.0",
        "title": "Capped", "license": "CC0-1.0", "kind": "web", "entry": "index.html", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" }, "web": { "storage_kib": 4 } });
    fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
    fs::write(source.join("index.html"), "<!DOCTYPE html>\n<html><body></body></html>").unwrap();
    let uids = ask(json!({ "op": "examples", "dir": source.parent().unwrap() }));
    let uid = uids[0].as_str().unwrap_or_else(|| panic!("{uids}")).to_owned();
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    (uid, path)
}

fn api(spec: &Value) -> String {
    let url = format!("{}:wprs/api.js", spec["url"].as_str().unwrap());
    serve(HANDLE, &url).unwrap()["text"].as_str().unwrap().to_owned()
}

pub(super) fn pages_keep_no_more_than_they_declared() {
    let (uid, path) = installed();
    let full = || ask(json!({ "op": "scene", "uid": uid, "languages": ["en"] }))["web"]["full"].clone();
    let spec = page::spec(HANDLE, &uid, &path, false, None).unwrap();
    let allowed = wprs_format::kept_allowance(4);
    assert_eq!((spec["storage"].clone(), full()), (json!(true), json!(false)));
    assert!(api(&spec).contains("QuotaExceededError") && api(&spec).contains(&format!("({allowed}, ")));
    assert!(spec["guard"].as_str().unwrap().contains("QuotaExceededError"), "frames are not held to the cap");
    for ignored in [r#"{"kept":"lots"}"#, r#"{"used":-1}"#, r#"{"kept":4096}"#] {
        assert!(script::heard(HANDLE, ignored).is_none());
        assert_eq!(page::spec(HANDLE, &uid, &path, false, None).unwrap()["key"], spec["key"], "{ignored}");
    }
    script::heard(HANDLE, &json!({ "used": allowed - 4095 }).to_string());
    let over = page::spec(HANDLE, &uid, &path, false, None).unwrap();
    assert_eq!((over["storage"].clone(), full()), (json!(false), json!(true)), "{over}");
    assert!(
        over["origin"].as_str().unwrap().ends_with(".e.wpak.invalid")
            && !api(&over).contains("QuotaExceeded")
            && !over["guard"].as_str().unwrap().contains("QuotaExceeded")
    );
    assert_eq!(ask(json!({ "op": "clear_data", "uid": uid })), Value::Null);
    assert!(ask(json!({ "op": "cleared" })).as_array().unwrap().contains(&spec["origin"]));
    let fresh = page::spec(HANDLE, &uid, &path, false, None).unwrap();
    assert_eq!((fresh["storage"].clone(), full()), (json!(true), json!(false)), "{fresh}");
    assert_ne!(fresh["origin"], spec["origin"]);
    page::close(HANDLE);
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
