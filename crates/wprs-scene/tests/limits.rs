use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::*;

const FULL: &str = include_str!("newer.json");

fn broken(changes: &[(&str, Json)]) -> Vec<String> {
    let mut doc: Json = serde_json::from_str(FULL).unwrap();
    for (pointer, value) in changes {
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        let target = doc.pointer_mut(parent).unwrap_or_else(|| panic!("{parent}"));
        match target {
            Json::Array(list) => list[key.parse::<usize>().unwrap()] = value.clone(),
            other => drop(other.as_object_mut().unwrap().insert(key.into(), value.clone())),
        }
    }
    let scene = Scene::from_json(&doc.to_string()).unwrap_or_else(|e| panic!("{changes:?}: {e}"));
    scene.validate(&BTreeMap::new()).iter().map(ToString::to_string).collect()
}

#[test]
fn new_fields_refuse_values_outside_their_limits() {
    let cases: Vec<(Vec<(&str, Json)>, &str)> = vec![
        (vec![("/canvas/camera/zoom", json!(20))], "/canvas/camera/zoom is 20, expected 0.1..=10"),
        (vec![("/canvas/hdr", json!(false))], "/canvas/tonemap does not fit the type"),
        (vec![("/canvas/perspective", json!(50))], "/canvas/perspective is 50, expected 100..=100000"),
        (vec![("/canvas/bloom/hdr/threshold", json!(20))], "/canvas/bloom/hdr/threshold is 20"),
        (vec![("/canvas/parallax/delay", json!(3))], "/canvas/parallax/delay is 3, expected 0..=2"),
        (vec![("/canvas/camera/shake/roughness", json!(2))], "/canvas/camera/shake/roughness is 2"),
        (vec![("/audio/release", json!(0))], "/audio/release is 0, expected 0.01..=10"),
        (vec![("/clock/step", json!(0.5))], "/clock/step is 0.5, expected 1..=3600"),
        (vec![("/assets/sky/path", json!("sky.ogg"))], "/assets/sky/filter does not fit"),
        (vec![("/assets/run/frames", json!(9))], "/assets/run/frames is 9, expected 1..=8"),
        (vec![("/assets/walk/durations", json!([0.1]))], "/assets/walk/durations is 1, expected 2..=2"),
        (vec![("/assets/walk/fps", json!(10))], "/assets/walk/durations does not fit"),
        (vec![("/assets/cat/path", json!("cat.png"))], "/assets/cat/animated does not fit"),
        (vec![("/assets/serif/path", json!("fonts/serif.ttf"))], "/assets/serif/face does not fit"),
        (vec![("/assets/normals/fps", json!(10))], "/assets/normals/fps does not fit"),
        (
            vec![("/assets/sky/path", json!("sky.ogg")), ("/assets/sky/filter", json!(null))],
            "/assets/sky/wrap does not fit",
        ),
        (vec![("/layers/0/brightness", json!(20))], "/layers/0/brightness is 20, expected 0..=16"),
        (
            vec![("/layers/0/transform/tilt", json!([95, 0]))],
            "/layers/0/transform/tilt/0 is 95, expected -89..=89",
        ),
        (vec![("/layers/0/parallax", json!([0.1, 20]))], "/layers/0/parallax/1 is 20"),
        (vec![("/layers/0/lighting/normal", json!("rain"))], "/layers/0/lighting/normal does not fit"),
        (vec![("/layers/0/mask/strength", json!(2))], "/layers/0/mask/strength is 2, expected 0..=1"),
        (vec![("/layers/0/parent", json!("runner"))], "/layers/0/parent depends on itself"),
        (vec![("/layers/1/inherit", json!(["opacity", "opacity"]))], "/layers/1/inherit does not fit"),
        (vec![("/layers/2/playback/end", json!(0.5))], "/layers/2/playback/end is 0.5"),
        (vec![("/layers/3/wrap/width", json!(0))], "/layers/3/wrap/width is 0, expected 1..=16384"),
        (vec![("/layers/3/fallback", json!(vec!["a"; 9]))], "/layers/3/fallback has 9 entries, at most 8"),
        (vec![("/layers/3/outline/width", json!(100))], "/layers/3/outline/width is 100, expected 0..=64"),
        (vec![("/layers/5/cone", json!([50, 40]))], "/layers/5/cone/1 is 40, expected 50..=180"),
        (vec![("/layers/6/cone", json!([10, 20]))], "/layers/6/cone does not fit"),
        (vec![("/layers/5/opacity", json!(0.5))], "/layers/5 is a light layer"),
        (vec![("/layers/7/size", json!([5000, 300]))], "/layers/7/size/0 is 5000, expected 1..=3840"),
        (vec![("/layers/8/sound", json!("wind"))], "/layers/8 needs either sound or sounds"),
        (vec![("/layers/8/pause", json!([9, 3]))], "/layers/8/pause/1 is 3, expected 9..=600"),
        (vec![("/layers/8/play_on", json!("event:nothing"))], "/layers/8/play_on names the unknown event"),
        (
            vec![("/layers/9/opacity/keys/0/2", json!({ "out": [3, 1] }))],
            "/layers/9/opacity/keys/0/2/out/0 is 3",
        ),
        (vec![("/layers/9/transform/position/r", json!(1))], "/layers/9/transform/position/r does not fit"),
        (
            vec![("/layers/9/transform/rotation/clock", json!("fortnight"))],
            "/layers/9/transform/rotation does not",
        ),
        (
            vec![("/layers/10/opacity/audio", json!([2, 20]))],
            "/layers/10/opacity/audio/1 is 20, expected 2..=15",
        ),
        (vec![("/layers/10/opacity/of", json!(12))], "/layers/10/opacity/of is 12"),
        (vec![("/layers/10/opacity/exponent", json!(20))], "/layers/10/opacity/exponent is 20"),
        (vec![("/layers/10/opacity/bounds", json!([0.8, 0.2]))], "/layers/10/opacity/bounds/1 is 0.2"),
        (vec![("/timelines/0/events/0/t", json!(9))], "/timelines/0/events/0/t is 9, expected 0..=4"),
        (vec![("/timelines/0/events/0/name", json!("Drop!"))], "/timelines/0/events/0/name must match"),
        (vec![("/effects/ripple/wgsl", json!("effects/ripple.txt"))], "/effects/ripple/wgsl does not fit"),
        (
            vec![("/effects/blur", json!({ "descriptor": "b.json", "wgsl": "b.wgsl" }))],
            "/effects/blur repeats an id",
        ),
        (
            vec![(
                "/layers/0/effects",
                json!([{ "effect": "depth_parallax", "params": { "depth": { "layer": "glow" } } }]),
            )],
            "/layers/0/effects depends on itself",
        ),
        (
            vec![(
                "/layers/0/effects",
                json!([{ "effect": "blur", "params": { "radius": { "layer": "glow" } } }]),
            )],
            "/layers/0/effects/0/params/radius does not fit",
        ),
        (
            vec![("/layers/9/visible", json!({ "prop": "x", "base": true }))],
            "/layers/9/visible/base does not fit",
        ),
    ];
    for (changes, expected) in cases {
        let issues = broken(&changes);
        assert!(issues.iter().any(|i| i.contains(expected)), "{changes:?} gave {issues:?}, not {expected}");
    }
    let lights: Vec<Json> =
        (0..13).map(|i| json!({ "id": format!("l{i}"), "type": "light", "light": "directional" })).collect();
    let issues = broken(&[("/layers", json!(lights))]);
    assert!(
        issues.iter().any(|i| i == "the scene has 13 light layers, at most 12 are allowed"),
        "{issues:?}"
    );
}

#[test]
fn malformed_new_fields_are_refused_while_parsing() {
    let mut doc: Json = serde_json::from_str(FULL).unwrap();
    for (pointer, value) in [
        ("/layers/0/blend", json!("tint")),
        ("/layers/5/light", json!("area")),
        ("/canvas/tonemap", json!("filmic")),
        ("/layers/3/render", json!("vector")),
        ("/assets/sky/wrap", json!("tile")),
        ("/layers/8/mode", json!("shuffle")),
        ("/layers/9/opacity/keys/0/2", json!({ "out": [0.5, 1], "tension": 1 })),
        ("/timelines/0/events/0", json!({ "t": 1, "name": "x", "at": 2 })),
    ] {
        let saved = doc.pointer(pointer).cloned().unwrap();
        *doc.pointer_mut(pointer).unwrap() = value;
        assert!(Scene::from_json(&doc.to_string()).is_err(), "{pointer} was accepted");
        *doc.pointer_mut(pointer).unwrap() = saved;
    }
    assert!(Scene::from_json(&doc.to_string()).is_ok());
}
