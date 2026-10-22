use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
    command.args(args).current_dir(dir).stdin(Stdio::null());
    for (key, sub) in [("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache"), ("XDG_CONFIG_HOME", "config")]
    {
        command.env(key, dir.join(sub));
    }
    command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}/no-bus", dir.display()));
    command.output().unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn fails(dir: &Path, args: &[&str], code: i32) -> String {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(code), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    String::from_utf8(out.stderr).unwrap()
}

#[test]
fn models_need_consent_and_their_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let list: Value = serde_json::from_str(&ok(dir, &["models", "list", "--json"])).unwrap();
    assert_eq!(
        (&list[0]["id"], &list[0]["license"], &list[0]["installed"]),
        (&json!("depth-anything-v2-small"), &json!("Apache-2.0"), &json!(false))
    );
    let urls = list[0]["urls"].as_array().unwrap();
    assert!(urls.len() == 1 && urls[0].as_str().unwrap().starts_with("https://github.com/"));
    let asked = fails(dir, &["models", "download", "depth-anything-v2-small"], 2);
    assert!(
        asked.contains("99.1 MB from github.com, license Apache-2.0") && asked.contains("--yes"),
        "{asked}"
    );
    std::fs::create_dir_all(dir.join("config/wallpaper-rs")).unwrap();
    std::fs::write(dir.join("config/wallpaper-rs/config.toml"), "[models]\nallow_download = false\n")
        .unwrap();
    assert!(
        fails(dir, &["models", "download", "depth-anything-v2-small", "--yes"], 1).contains("turned off")
    );
    std::fs::write(dir.join("fake.onnx"), b"not a model").unwrap();
    let refused = fails(dir, &["models", "install", "depth-anything-v2-small", "--file", "fake.onnx"], 1);
    assert!(refused.contains("SHA-256 differs"), "{refused}");
    assert!(fails(dir, &["models", "remove", "depth-anything-v2-large"], 2).contains("is not one of"));
    assert!(ok(dir, &["models", "list"]).contains("not installed"));
}
