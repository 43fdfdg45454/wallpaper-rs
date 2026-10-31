use super::*;

#[test]
fn the_schema_describes_every_new_field() {
    let schema = schema().to_string();
    let words = [
        "camera",
        "shake",
        "bloom",
        "tonemap",
        "perspective",
        "ambient",
        "sky",
        "filter",
        "mipmaps",
        "face",
        "rects",
        "durations",
        "animated",
        "brightness",
        "parent",
        "inherit",
        "lighting",
        "tilt",
        "playback",
        "fallback",
        "valign",
        "wrap",
        "line_height",
        "letter_spacing",
        "outline",
        "shadow",
        "render",
        "volumetric",
        "cone",
        "sounds",
        "pause",
        "stream",
        "play_on",
        "events",
        "base",
        "bounds",
        "exponent",
        "reduce",
        "strength",
        "redraw",
        "light",
        "linear_burn",
        "negation",
        "sdf",
        "agx",
        "attack",
        "release",
        "descriptor",
        "wgsl",
    ];
    for word in words {
        assert!(schema.contains(&format!("\"{word}\"")), "the scene schema lacks {word}");
    }
}

#[test]
fn every_newer_key_counts_whatever_its_value() {
    let base: Json = serde_json::from_str(SAMPLE).unwrap();
    assert!(!newer_scene_keys(&base));
    assert!(newer_scene_keys(&serde_json::from_str(FULL).unwrap()));
    let keys = [
        ("", "effects", json!({})),
        ("", "pools", json!({})),
        ("", "templates", json!({})),
        ("/canvas", "hdr", json!(false)),
        ("/canvas", "clear", json!(true)),
        ("/clock", "step", json!(null)),
        ("/clock", "max_per_step", json!(null)),
        ("/assets/sky", "rects", json!([])),
        ("/assets/sky", "durations", json!([])),
        ("/assets/sky", "animated", json!(false)),
        ("/assets/sky", "mipmaps", json!(null)),
        ("/scripts/0", "uses", json!([])),
        ("/scripts/0", "events", json!([])),
        ("/scripts/0", "params", json!({})),
        ("/particles/sparks", "children", json!([])),
        ("/particles/sparks", "space", json!("local")),
        ("/timelines/0", "wrap", json!(false)),
        ("/timelines/0", "events", json!([])),
        ("/layers/0", "redraw", json!(false)),
        ("/layers/0", "rgb", json!(false)),
        ("/layers/4", "space", json!("2d")),
        ("/layers/6", "depth_test", json!(false)),
    ];
    for (at, key, value) in keys {
        let mut scene = base.clone();
        scene.pointer_mut(at).and_then(Json::as_object_mut).unwrap().insert(key.into(), value);
        assert!(newer_scene_keys(&scene), "{at}/{key}");
    }
}
