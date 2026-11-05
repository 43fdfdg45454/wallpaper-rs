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

#[test]
fn every_field_of_a_particle_system_is_set_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    glow(dir);
    ok(dir, &["scene", "add-layer", "glow", "particles", "--id", "dust"]);
    let pt = |args: &[&'static str]| [&["scene", "particles", "glow", "dust"][..], args].concat();
    ok(dir, &["scene", "particles", "glow", "sparks", "create", "--parent", "dust", "--on", "static"]);
    assert_eq!(shown(dir, "/scene/particles/dust/children/0"), json!({"system": "sparks", "on": "static"}));
    ok(
        dir,
        &pt(&[
            "settings",
            "--capacity",
            "300",
            "--seed",
            "4",
            "--prewarm",
            "1.5",
            "--space",
            "world",
            "--rate",
            "25",
        ]),
    );
    assert_eq!(shown(dir, "/scene/particles/dust/emitters/0/rate"), json!(25.0));
    ok(
        dir,
        &pt(&["start", "--life", "1,3", "--size", "2,5", "--color", "#ffffff,#88ccff", "--alpha", "0.5,1"]),
    );
    ok(dir, &pt(&["start", "--velocity", "-10,0:10,-40", "--spin", "-90,90", "--clear", "alpha"]));
    assert_eq!(shown(dir, "/scene/particles/dust/init/velocity"), json!([[-10.0, 0.0], [10.0, -40.0]]));
    ok(dir, &pt(&["force", "gravity", "0,40"]));
    ok(dir, &pt(&["force", "drag", "0.2", "--index", "0"]));
    ok(dir, &pt(&["force", "oscillate", "size", "--freq", "1,2", "--scale", "0.5,1"]));
    ok(
        dir,
        &pt(&["force", "attractor", "cursor", "--strength", "-60", "--radius", "120", "--falloff", "linear"]),
    );
    ok(dir, &pt(&["force", "turbulence", "--strength", "10", "--scale", "0.01"]));
    assert_eq!(shown(dir, "/scene/particles/dust/forces/0"), json!({"drag": 0.2}));
    ok(dir, &pt(&["force", "remove", "3"]));
    ok(dir, &pt(&["child", "add", "sparks", "--on", "death", "--probability", "0.5", "--offset", "0,-4"]));
    ok(dir, &pt(&["child", "set", "1", "sparks", "--on", "spawn", "--max", "3"]));
    ok(dir, &pt(&["child", "remove", "0"]));
    assert_eq!(
        shown(dir, "/scene/particles/dust/children"),
        json!([{"system": "sparks", "on": "spawn", "max": 3}])
    );
    ok(dir, &pt(&["size-over-life", "0:1", "0.5:2", "1:0"]));
    ok(dir, &pt(&["color-over-life", "0:#ffffff", "1:#ff000000"]));
    ok(dir, &pt(&["render", "--stretch", "0.1,2,40", "--frames", "random", "--brightness", "2.5"]));
    assert_eq!(
        shown(dir, "/scene/particles/dust/render/stretch"),
        json!({"time": 0.1, "min": 2.0, "max": 40.0})
    );
    ok(dir, &pt(&["size-over-life", "none"]));
    assert_eq!(shown(dir, "/scene/particles/dust/render/size_over_life"), Value::Null);
    assert!(fails(dir, &pt(&["settings", "--prewarm", "900"])).contains("prewarm"));
    assert!(fails(dir, &pt(&["render", "--brightness", "99"])).contains("brightness"));
    assert_eq!(super::run(dir, &pt(&["size-over-life", "half"])).status.code(), Some(2));
    assert!(fails(dir, &["scene", "particles", "glow", "sparks", "delete"]).contains("sparks"));
    ok(dir, &["scene", "particles", "glow", "lone", "create"]);
    ok(dir, &["scene", "particles", "glow", "lone", "delete"]);
}
