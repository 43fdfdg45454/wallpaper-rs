use super::authoring::{glow, shown};
use super::{fails, ok};
use serde_json::json;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

#[test]
fn a_mesh_layer_comes_from_a_glb_and_a_new_clip_plays_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    glow(dir);
    let glb = format!("{FIXTURES}/parts.glb");
    assert_eq!(ok(dir, &["scene", "add-layer", "glow", "mesh", "--id", "body", "--file", &glb]), "body\n");
    assert_eq!(shown(dir, "/scene/layers/2/mesh"), "parts");
    ok(dir, &["scene", "clip", "glow", "body", "wave", "--add", "--play"]);
    assert_eq!(shown(dir, "/scene/layers/2/animations"), json!([{"clip": "wave"}]));
    let picture = format!("{FIXTURES}/green-64x32.png");
    assert!(fails(dir, &["scene", "add-layer", "glow", "mesh", "--file", &picture]).contains("cannot show"));
    assert_eq!(super::run(dir, &["scene", "clip", "glow", "body", "wave", "--play"]).status.code(), Some(2));
}

#[test]
fn timelines_relative_keys_and_handles_are_written_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    glow(dir);
    let opacity = "/scene/layers/0/opacity";
    let missing = fails(dir, &["scene", "keyframe", "glow", opacity, "0", "1", "--timeline", "intro"]);
    assert!(missing.contains("timeline"), "{missing}");
    ok(
        dir,
        &["scene", "timeline", "glow", "intro", "--add", "--frames", "48", "--fps", "24", "--loop", "repeat"],
    );
    assert_eq!(
        shown(dir, "/scene/timelines/0"),
        json!({"id": "intro", "frames": 48, "fps": 24.0, "loop": "repeat"})
    );
    ok(dir, &["scene", "keyframe", "glow", opacity, "0", "1", "--timeline", "intro"]);
    ok(dir, &["scene", "keyframe", "glow", opacity, "2", "-0.5", "--base", "0.75"]);
    ok(dir, &["scene", "keyframe", "glow", opacity, "0", "--out-handle", "0.5,0"]);
    assert_eq!(shown(dir, &format!("{opacity}/keys/0/2")), json!({"out": [0.5, 0.0]}));
    assert_eq!(shown(dir, &format!("{opacity}/base")), json!(0.75));
    assert!(
        fails(dir, &["scene", "keyframe", "glow", opacity, "0", "--ease", "linear", "--in-handle", "1,0"])
            .contains("not both")
    );
    assert!(fails(dir, &["scene", "timeline", "glow", "intro", "--speed", "900"]).contains("speed"));
    ok(dir, &["scene", "timeline", "glow", "intro", "--rename", "opening", "--duration", "3"]);
    assert_eq!(shown(dir, &format!("{opacity}/timeline")), "opening");
    ok(dir, &["scene", "timeline", "glow", "opening", "--remove"]);
    assert_eq!(shown(dir, &format!("{opacity}/timeline")), serde_json::Value::Null);
    ok(dir, &["scene", "keyframe", "glow", opacity, "0", "--absolute"]);
    assert_eq!(shown(dir, &format!("{opacity}/base")), serde_json::Value::Null);
}
