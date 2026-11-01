use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};
use wprs_core::stash::{EVERY, Stash, file, forget};

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
    stash.take("DP-1", &json!({ "package": { "visits": 1 }, "output": { "seat": 2 } }));
    stash.flush(now, false).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    stash.take("DP-1", &json!({ "package": { "visits": 2 } }));
    stash.flush(now + Duration::from_secs(1), false).unwrap();
    let read = |p: &std::path::Path| {
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(p).unwrap()).unwrap()
    };
    assert_eq!(read(&path)["package"]["visits"], 1, "a second write waits");
    assert!(stash.due(now + EVERY));
    stash.take("DP-1", &json!({ "package": { "big": "x".repeat(70_000) } }));
    for i in 0..20 {
        stash.take(&format!("HDMI-{i}"), &json!({ "output": { "n": i } }));
    }
    drop(stash);
    let kept = read(&path);
    assert_eq!(kept["package"]["visits"], 2, "stopping writes the rest, and too much is refused");
    assert_eq!(kept["outputs"].as_object().unwrap().len(), 16);
    let again = Stash::open(dir.path(), "org.example.sea").unwrap();
    assert_eq!(again.scopes("HDMI-19")["output"]["n"], 19);
    drop(again);
    forget(dir.path(), "org.example.sea").unwrap();
    assert!(!path.exists() && forget(dir.path(), "org.example.sea").is_ok());
}
