use super::{fails, ok};
use serde_json::{Value, json};
use std::path::Path;

pub fn shown(dir: &Path, pointer: &str) -> Value {
    let out = super::run(dir, &["scene", "show", "glow", pointer]);
    serde_json::from_slice(&out.stdout).unwrap_or(Value::Null)
}

pub fn glow(dir: &Path) {
    ok(dir, &["scene", "new", "glow"]);
    ok(dir, &["scene", "add-layer", "glow", "solid", "--id", "card"]);
    ok(dir, &["scene", "add-layer", "glow", "solid", "--id", "hole"]);
}

#[test]
fn effects_are_stacked_set_masked_and_removed_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    glow(dir);
    let fx = |args: &[&'static str]| [&["scene", "effects", "glow", "card"][..], args].concat();
    ok(dir, &fx(&["add", "vignette", "--param", "intensity=0.5", "--name", "edge"]));
    ok(dir, &fx(&["add", "film_grain", "--at", "0"]));
    assert_eq!(shown(dir, "/scene/layers/0/effects/1/params"), json!({"intensity": 0.5}));
    ok(dir, &fx(&["set", "1", "--param", "radius=1.5", "--unset", "intensity", "--enabled", "false"]));
    assert_eq!(shown(dir, "/scene/layers/0/effects/1/params"), json!({"radius": 1.5}));
    ok(dir, &fx(&["mask", "1", "--shape", r#"{"rect": {"size": [100, 50]}}"#, "--feather", "8"]));
    ok(dir, &fx(&["mask", "1", "--from", "hole", "--channel", "luminance"]));
    assert_eq!(shown(dir, "/scene/layers/0/effects/1/mask/layer"), "hole");
    ok(dir, &fx(&["move", "1", "0"]));
    assert_eq!(shown(dir, "/scene/layers/0/effects/0/effect"), "vignette");
    assert!(fails(dir, &fx(&["set", "0", "--param", "intensity=7"])).contains("intensity"));
    assert!(fails(dir, &fx(&["add", "sparkle_dust"])).contains("sparkle_dust"));
    let selfish = fails(dir, &fx(&["mask", "0", "--from", "card"]));
    assert!(selfish.contains("itself"), "{selfish}");
    assert_eq!(super::run(dir, &fx(&["add", "vignette", "--param", "loose"])).status.code(), Some(2));
    ok(dir, &fx(&["mask", "0", "--off"]));
    ok(dir, &fx(&["remove", "1"]));
    ok(dir, &fx(&["remove", "0"]));
    assert_eq!(shown(dir, "/scene/layers/0/effects"), Value::Null);
    ok(dir, &["scene", "add", "glow", "/scene/layers/1/effects/-", r#"{"effect": "vignette"}"#]);
    assert_eq!(shown(dir, "/scene/layers/1/effects/0/effect"), "vignette");
}
