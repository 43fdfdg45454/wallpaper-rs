use super::{fails, ok, show};
use serde_json::json;

#[test]
fn timelines_and_sounds_start_on_media_triggers_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "snow", "--from", "empty"]);
    ok(dir, &["scene", "add", "snow", "/scene/timelines", r#"[{"id": "song", "duration": 2}]"#]);
    let sound = json!({ "id": "ding", "type": "sound", "sound": "bell" }).to_string();
    ok(dir, &["scene", "add", "snow", "/scene/layers/-", &sound]);
    assert_eq!(ok(dir, &["scene", "trigger", "snow", "song"]), "none\n");
    let offered = ok(dir, &["scene", "trigger", "snow", "song", "--list"]);
    let media: Vec<&str> = offered.lines().filter(|l| l.starts_with("media:")).collect();
    assert_eq!(media, ["media:play", "media:pause", "media:track", "media:cover"]);
    ok(dir, &["scene", "trigger", "snow", "song", "media:track"]);
    assert_eq!(show(dir, "/scene/timelines/0/trigger"), "media:track");
    ok(dir, &["scene", "trigger", "snow", "ding", "media:play"]);
    let layers = show(dir, "/scene/layers");
    let ding = layers.as_array().unwrap().iter().find(|l| l["id"] == "ding").unwrap();
    assert_eq!(ding["play_on"], "media:play");
    assert_eq!(ok(dir, &["scene", "trigger", "snow", "ding"]), "media:play\n");
    assert!(fails(dir, &["scene", "trigger", "snow", "song", "media:skip"]).contains("not a media trigger"));
    assert!(
        fails(dir, &["scene", "trigger", "snow", "nothing", "beat"]).contains("no timeline or sound layer")
    );
    ok(dir, &["scene", "trigger", "snow", "song", "--none"]);
    assert_eq!(ok(dir, &["scene", "trigger", "snow", "song"]), "none\n");
    ok(dir, &["scene", "undo", "snow"]);
    assert_eq!(show(dir, "/scene/timelines/0/trigger"), "media:track");
}
