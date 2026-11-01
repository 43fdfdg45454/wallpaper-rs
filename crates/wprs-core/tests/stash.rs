use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};
use wprs_core::stash::{EVERY, Keeper, Stash, file, forget};

fn read(p: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn set(seq: u64, package: Value, output: Value) -> Value {
    json!({ "seq": seq, "set": { "package": package, "output": output } })
}

#[test]
fn script_storage_is_private_bounded_and_written_at_most_every_five_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path(), "org.example.sea").unwrap();
    assert!(
        file(dir.path(), "../sea").is_none()
            && file(dir.path(), ".hidden").is_none()
            && file(dir.path(), "").is_none()
    );
    let now = Instant::now();
    let mut stash = Stash::open(dir.path(), "org.example.sea").unwrap();
    assert_eq!(stash.scopes("DP-1"), json!({ "package": {}, "output": {} }));
    assert!(stash.merge("DP-1", &set(1, json!({ "visits": 1 }), json!({ "seat": 2 }))));
    stash.flush(now, false).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    stash.merge("DP-1", &set(2, json!({ "visits": 2 }), json!({})));
    stash.flush(now + Duration::from_secs(1), false).unwrap();
    assert_eq!(read(&path)["package"]["visits"], 1, "a second write waits");
    assert!(stash.due(now + EVERY));
    assert!(!stash.merge("DP-1", &set(3, json!({ "big": "x".repeat(70_000) }), json!({}))));
    for i in 0..20 {
        stash.merge(&format!("HDMI-{i}"), &set(4, json!({}), json!({ "n": i })));
    }
    stash.merge("HDMI-4", &json!({ "gone": { "output": ["n"] } }));
    drop(stash);
    let kept = read(&path);
    assert_eq!(kept["package"]["visits"], 2, "stopping writes the rest, and too much is refused");
    let outputs = kept["outputs"].as_object().unwrap();
    assert_eq!(outputs.len(), 16);
    assert!(!outputs.contains_key("DP-1") && !outputs.contains_key("HDMI-3"), "the oldest screens go first");
    assert_eq!(outputs["HDMI-4"], json!({}));
    let again = Stash::open(dir.path(), "org.example.sea").unwrap();
    assert_eq!(again.scopes("HDMI-19")["output"]["n"], 19);
    drop(again);
    forget(dir.path(), "org.example.sea").unwrap();
    assert!(!path.exists() && forget(dir.path(), "org.example.sea").is_ok());
}

#[test]
fn screens_of_one_package_share_one_store_and_hear_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path(), "org.example.tide").unwrap();
    let now = Instant::now();
    let mut a = Keeper::open(dir.path(), "org.example.tide", "A", true).unwrap();
    let mut b = Keeper::open(dir.path(), "org.example.tide", "B", true).unwrap();
    let mut preview = Keeper::open(dir.path(), "org.example.tide", "preview:1", false).unwrap();
    a.take(&set(1, json!({ "shared": "A" }), json!({ "k": "fromA" })), now);
    b.take(&set(1, json!({ "other": "B" }), json!({ "k": "fromB" })), now);
    preview.take(&set(1, json!({ "shared": "preview" }), json!({ "k": "preview" })), now);
    let before = a.frame().0;
    a.refresh(now);
    let heard: Value = serde_json::from_str(a.frame().1).unwrap();
    assert!(a.frame().0 > before, "a change from another screen reaches the script");
    assert_eq!(
        heard,
        json!({ "package": { "shared": "A", "other": "B" }, "output": { "k": "fromA" }, "seq": 1 })
    );
    let quiet = a.frame().0;
    a.refresh(now);
    assert_eq!(a.frame().0, quiet, "nothing new, nothing sent");
    drop((a, b, preview));
    let kept = read(&path);
    assert_eq!(kept["package"], json!({ "shared": "A", "other": "B" }));
    assert_eq!(kept["outputs"], json!({ "A": { "k": "fromA" }, "B": { "k": "fromB" } }));
}

#[test]
fn a_forgotten_package_is_not_written_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path(), "org.example.gone").unwrap();
    let mut a = Keeper::open(dir.path(), "org.example.gone", "A", true).unwrap();
    a.take(&set(1, json!({ "n": 1 }), json!({})), Instant::now());
    assert!(path.exists());
    a.take(&set(2, json!({ "n": 2 }), json!({})), Instant::now());
    forget(dir.path(), "org.example.gone").unwrap();
    drop(a);
    assert!(!path.exists());
    let fresh = Keeper::open(dir.path(), "org.example.gone", "A", true).unwrap();
    assert_eq!(fresh.scopes(), json!({ "package": {}, "output": {} }));
}
