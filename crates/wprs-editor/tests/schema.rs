use serde_json::{Value, json};
use wprs_editor::inspect::fields;
use wprs_editor::schema::{MANIFEST, PROPERTY, child, fits};
use wprs_editor::starters::starter;
use wprs_format::Manifest;

fn deep(root: &Value, schema: &Value, value: &Value, at: &str) {
    assert!(fits(root, schema, value), "{at} does not fit its schema: {value}");
    let children: Vec<(String, &Value)> = match value {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(list) => list.iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect(),
        _ => Vec::new(),
    };
    for (key, item) in children {
        let next = child(root, schema, &key, value).unwrap_or_else(|| panic!("{at}/{key} has no schema"));
        deep(root, &next, item, &format!("{at}/{key}"));
    }
}

fn properties() -> Value {
    json!({
        "tint": { "type": "color", "default": "#ff9a3d", "label": "@tint" },
        "speed": { "type": "slider", "min": 0, "max": 2, "decimals": 2, "editable": true, "default": 1, "quality": { "q1": 0.5 } },
        "photos": { "type": "folder", "accept": ["image"], "mode": "all", "max_files": 500, "recursive": 1 },
        "open": { "type": "shortcut", "icon": "folder" }
    })
}

#[test]
fn every_new_manifest_field_round_trips_through_its_schema() {
    let web = json!({
        "schema": "wallpaper-rs.wpak/2", "id": "org.example.page", "version": "1.0.0", "kind": "web", "entry": "index.html",
        "title": "@title", "default_locale": "en", "strings": { "en": { "title": "Page", "tint": "Tint" } },
        "license": "CC-BY-4.0", "content": { "rating": "general" }, "photosensitive": false,
        "origin": { "kind": "provider", "source_url": "https://example.org/1",
                    "provider": { "name": "example", "item_url": "https://example.org/1", "credit": "Jo", "terms_url": "https://example.org/terms" } },
        "credits": [{ "paths": ["app.js"], "license": "MIT", "attribution": "Jo", "source": { "id": "org.example.fx", "version": "1.2.0" } }],
        "requires": { "engine": ">=0.3", "features": ["web"], "packages": [{ "id": "org.example.fx", "version": "^1.2" }], "gpu": "compute" },
        "platforms": ["desktop"], "inputs": ["audio", "network"],
        "permissions": { "network": { "allow": ["https://api.example.org"] } },
        "web": { "preload": "compat.js", "background": "#000000", "storage_kib": 512 },
        "playback": { "max_fps": "display", "volume": 0.5, "muted": true, "loop": false },
        "display": { "accent": { "prop": "tint" } },
        "properties": properties()
    });
    let video = json!({
        "schema": "wallpaper-rs.wpak/2", "id": "org.example.rain", "version": "1.0.0", "kind": "video", "entry": "rain.webm",
        "title": "Rain", "license": "CC0-1.0", "content": { "rating": "general" }, "photosensitive": false,
        "origin": { "kind": "imported" }, "playback": { "max_fps": 30 },
        "display": { "fit": "span", "background": "#101010", "span": "all-outputs", "flip": "horizontal", "accent": "#336699" }
    });
    for value in [web, video] {
        deep(&MANIFEST, &MANIFEST, &value, "");
        let manifest: Manifest = serde_json::from_value(value.clone()).unwrap();
        manifest.check().unwrap();
        let written = serde_json::to_value(&manifest).unwrap();
        deep(&MANIFEST, &MANIFEST, &written, "");
        assert_eq!(serde_json::from_value::<Manifest>(written).unwrap(), manifest);
    }
    for (id, property) in properties().as_object().unwrap() {
        deep(&PROPERTY, &PROPERTY, property, id);
    }
}

#[test]
fn the_inspector_shows_the_new_fields_without_being_told() {
    let mut doc = starter("empty", "org.example.kinds", "Kinds", "en").unwrap();
    doc["manifest"]["display"] = json!({ "accent": "#336699" });
    doc["manifest"]["properties"] = properties();
    let manifest = fields(&doc, "/manifest");
    for key in ["platforms", "display", "requires"] {
        assert!(manifest.iter().any(|f| f.key == key), "the manifest shows no {key}");
    }
    assert!(manifest.iter().all(|f| !["web", "permissions", "playback"].contains(&f.key.as_str())));
    let display = fields(&doc, "/manifest/display");
    assert!(display.iter().any(|f| f.key == "span" && f.widget == "enum"), "{display:?}");
    assert!(display.iter().all(|f| f.key != "flip"), "scenes are not mirrored: {display:?}");
    let mut picture = doc.clone();
    picture["manifest"]["kind"] = json!("image");
    let display = fields(&picture, "/manifest/display");
    assert!(display.iter().any(|f| f.key == "flip" && f.widget == "enum"), "{display:?}");
    let folder = fields(&doc, "/manifest/properties/photos");
    assert!(folder.iter().any(|f| f.key == "max_files" && f.range == json!([1, 1000])));
    assert!(folder.iter().any(|f| f.key == "mode" && f.widget == "enum"));
}

#[test]
fn every_new_scene_field_round_trips_and_shows_in_the_inspector() {
    use wprs_editor::schema::SCENE;
    let scene: Value = serde_json::from_str(include_str!("../../wprs-scene/tests/newer.json")).unwrap();
    deep(&SCENE, &SCENE, &scene, "");
    let parsed = wprs_scene::Scene::from_json(&scene.to_string()).unwrap();
    deep(&SCENE, &SCENE, &serde_json::to_value(&parsed).unwrap(), "");
    let mut doc = starter("empty", "org.example.newer", "Newer", "en").unwrap();
    doc["scene"] = scene;
    let keys = |pointer: &str| fields(&doc, pointer).into_iter().map(|f| f.key).collect::<Vec<_>>();
    let shown = [
        (
            "/scene/canvas",
            &["parallax", "camera", "bloom", "hdr", "tonemap", "clear", "perspective", "ambient"][..],
        ),
        ("/scene/layers/0", &["brightness", "color", "parent", "lighting", "playback"]),
        ("/scene/layers/3", &["fallback", "valign", "wrap", "line_height", "outline", "shadow", "render"]),
        ("/scene/layers/5", &["light", "intensity", "radius", "cone", "volumetric", "texture"]),
        ("/scene/layers/8", &["sounds", "mode", "pause", "stream", "play_on"]),
        ("/scene/assets/run", &["filter", "wrap", "mipmaps", "frames", "fps", "durations", "animated"]),
        ("/scene/timelines/0", &["events"]),
    ];
    for (pointer, wanted) in shown {
        let found = keys(pointer);
        assert!(wanted.iter().all(|k| found.iter().any(|f| f == k)), "{pointer} shows {found:?}");
    }
    let blend = fields(&doc, "/scene/layers/0").into_iter().find(|f| f.key == "blend").unwrap();
    assert!(blend.options.contains(&json!("phoenix")) && blend.options.len() == 32);
}

#[test]
fn advanced_scene_fields_round_trip_and_show_in_the_inspector() {
    use wprs_editor::schema::SCENE;
    let scene: Value = serde_json::from_str(include_str!("../../wprs-scene/tests/deep.json")).unwrap();
    deep(&SCENE, &SCENE, &scene, "");
    let parsed = wprs_scene::Scene::from_json(&scene.to_string()).unwrap();
    deep(&SCENE, &SCENE, &serde_json::to_value(&parsed).unwrap(), "");
    let mut doc = starter("empty", "org.example.deep", "Deep", "en").unwrap();
    doc["scene"] = scene;
    doc["manifest"]["permissions"] = json!({ "rgb": true });
    doc["manifest"]["display"] = json!({ "span": "all-outputs" });
    deep(&MANIFEST, &MANIFEST, &doc["manifest"], "/manifest");
    let keys = |pointer: &str| fields(&doc, pointer).into_iter().map(|f| f.key).collect::<Vec<_>>();
    let shown = [
        (
            "/scene/layers/0",
            &["mesh", "clips", "animations", "ik", "physics", "shapes", "parts", "depth", "hit"][..],
        ),
        ("/scene/layers/1", &["attach"]),
        ("/scene/layers/2", &["space"]),
        ("/scene/layers/2/layers/0", &["model", "materials", "root_motion", "position", "rotation", "env"]),
        ("/scene/layers/3", &["instance", "render"]),
        ("/scene/layers/5", &["rgb"]),
        ("/scene/camera3d", &["fov", "near", "far", "eye", "paths", "order", "fade"]),
        ("/scene/particles/fireworks", &["space", "points", "perspective", "children", "lock"]),
        (
            "/scene/particles/fireworks/emitters/0",
            &["duration", "delay", "periodic", "max_per_step", "audio_gate"],
        ),
        (
            "/scene/particles/fireworks/init",
            &["alpha", "palette", "velocity", "turbulent", "offset_noise", "remap"],
        ),
        ("/scene/scripts/0", &["events", "uses", "params"]),
        ("/scene/pools/waves", &["max", "max_vertices", "texture"]),
    ];
    for (pointer, wanted) in shown {
        let found = keys(pointer);
        assert!(wanted.iter().all(|k| found.iter().any(|f| f == k)), "{pointer} shows {found:?}");
    }
    let space = fields(&doc, "/scene/layers/2").into_iter().find(|f| f.key == "space").unwrap();
    assert_eq!((space.widget.as_str(), space.options.len()), ("enum", 2));
    doc["scene"]["particles"]["embers"]["init"] = json!({ "life": [1, 2] });
    let init = fields(&doc, "/scene/particles/embers/init");
    let widget = |key: &str| init.iter().find(|f| f.key == key).map(|f| f.widget.clone()).unwrap_or_default();
    assert_eq!((widget("life").as_str(), widget("speed").as_str()), ("vec2", "vec2"), "{init:?}");
}

#[test]
fn released_layer_fields_keep_their_slider_and_switch() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenes");
    for name in ["dusk", "orbits", "pond", "pulse", "snowfall"] {
        let text = std::fs::read_to_string(format!("{root}/{name}/scene.json")).unwrap();
        let mut doc = starter("empty", "org.example.scene", "Scene", "en").unwrap();
        doc["scene"] = serde_json::from_str(&text).unwrap();
        let found = fields(&doc, "/scene/layers/0");
        let widget = |key: &str| found.iter().find(|f| f.key == key).map(|f| f.widget.clone());
        assert_eq!(
            (widget("parallax"), widget("solid")),
            (Some("number".into()), Some("bool".into())),
            "{name}"
        );
    }
}

#[test]
fn choices_of_described_effects_show_their_names() {
    let mut doc = starter("empty", "org.example.rays", "Rays", "en").unwrap();
    doc["scene"]["layers"] = json!([{ "id": "sun", "type": "solid", "color": "#ffffff",
        "effects": [{ "effect": "god_rays", "params": { "mode": 2 } }] }]);
    let found = fields(&doc, "/scene/layers/0/effects/0");
    let mode = found.iter().find(|f| f.key == "mode").unwrap();
    assert_eq!((mode.widget.as_str(), mode.names.len()), ("enum", mode.options.len()));
    assert!(mode.names.iter().all(|n| !n.is_empty() && n.parse::<f32>().is_err()), "{:?}", mode.names);
}
