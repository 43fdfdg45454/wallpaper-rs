mod common;

use common::{archive, image, manifest as v1, package, v2};
use image::ImageFormat;
use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, RgbaImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use wprs_format::{
    Content, Error, Input, Kind, Limits, Manifest, Origin, Previews, Rating, build, features, fill, media,
    open, read, rename_json, upgrade, upgrade_json,
};

const SCENE: &str = r##"{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [3840, 1600]},
    "layers": [{"id": "glow", "type": "solid", "color": "#336699", "opacity": {"audio": "bass"}}]}"##;

fn source(dir: &Path, manifest: &Manifest) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    fs::write(dir.join("scene.json"), SCENE).unwrap();
    fs::write(dir.join("preview.png"), image(320, 180, ImageFormat::Png)).unwrap();
}

fn gif(frames: u32, centis: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let frame = |_| Frame::from_parts(RgbaImage::new(8, 4), 0, 0, Delay::from_numer_denom_ms(centis * 10, 1));
    GifEncoder::new(&mut out).encode_frames((0..frames).map(frame)).unwrap();
    out
}

fn animated_webp(millis: [u32; 2]) -> Vec<u8> {
    let chunk = |name: &[u8], data: Vec<u8>| [name, &(data.len() as u32).to_le_bytes(), &data].concat();
    let anmf = |ms: u32| chunk(b"ANMF", [vec![0; 12], ms.to_le_bytes()[..3].to_vec(), vec![0]].concat());
    let vp8x = chunk(
        b"VP8X",
        [vec![2, 0, 0, 0], 99u32.to_le_bytes()[..3].to_vec(), 49u32.to_le_bytes()[..3].to_vec()].concat(),
    );
    let body = [b"WEBP".to_vec(), vp8x, anmf(millis[0]), anmf(millis[1])].concat();
    [b"RIFF".to_vec(), (body.len() as u32).to_le_bytes().to_vec(), body].concat()
}

#[test]
fn headers_give_size_and_length() {
    for (format, name) in [
        (ImageFormat::Png, "png"),
        (ImageFormat::Jpeg, "jpeg"),
        (ImageFormat::WebP, "webp"),
        (ImageFormat::Gif, "gif"),
    ] {
        let found = media::probe(&image(321, 123, format)).unwrap();
        assert_eq!((found.format, found.width, found.height), (name, 321, 123), "{name}");
    }
    assert_eq!(media::probe(&gif(3, 50)).map(|m| m.duration_ms), Some(1500));
    assert_eq!(
        media::probe(&animated_webp([400, 600])).map(|m| (m.width, m.height, m.duration_ms)),
        Some((100, 50, 1000))
    );
    assert_eq!(media::probe(b"RIFF\0\0\0\0WEBP"), None);
    assert_eq!(media::probe(b"plain text"), None);
}

#[test]
fn build_fills_required_features_and_checks_inputs() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, out) = (tmp.path().join("src"), tmp.path().join("glow.wpak"));
    let mut manifest = v2(Kind::Scene);
    source(&src, &manifest);
    let missing = build(&src, &out, &Limits::default());
    assert!(matches!(&missing, Err(Error::Invalid { field, .. }) if field == "inputs"), "{missing:?}");
    let at = missing.unwrap_err().to_string();
    assert!(at.contains("audio at /layers/0 of scene.json"), "{at}");
    manifest.inputs = vec![Input::Audio, Input::Media];
    source(&src, &manifest);
    assert_eq!(fill(&src).unwrap(), ["inputs.audio", "inputs.media"]);
    assert!(fill(&src).unwrap().is_empty());
    build(&src, &out, &Limits::default()).unwrap();
    let package = open(&out, &Limits::default()).unwrap();
    assert_eq!(package.manifest.features(), ["inputs.audio", "inputs.media"]);
    let facets = ["1440p", "android", "animated", "audio", "desktop", "landscape", "ultrawide"];
    assert_eq!(package.facts.facets, facets);
    let unused = ["inputs declares media but nothing in the package uses it"];
    assert_eq!(features::warnings(&package.manifest, &package.facts), unused);
    fs::write(src.join("manifest.json.minisig"), "stale").unwrap();
    source(&src, &manifest);
    assert!(matches!(fill(&src), Err(Error::Invalid { field, .. }) if field == "requires.features"));
}

#[test]
fn readers_check_features_and_previews() {
    let mut manifest = v2(Kind::Image);
    let png = image(64, 64, ImageFormat::Png);
    let load = |m: &Manifest, preview: &[u8]| {
        read(Cursor::new(package(m, &[("preview.png", preview)])), &Limits::default())
    };
    assert_eq!(load(&manifest, &png).unwrap().facts.facets, ["android", "desktop"]);
    let listed = ["inputs.audio", "inputs.network", "inputs.pointer"].map(String::from).to_vec();
    let requires = Some(wprs_format::Requires { features: listed, ..Default::default() });
    let inputs = vec![Input::Audio, Input::Pointer, Input::Network];
    let claims = Manifest { inputs, requires, ..manifest.clone() };
    let claimed = load(&claims, &png).unwrap();
    assert_eq!(claimed.facts.facets, ["android", "desktop"]);
    assert_eq!(features::warnings(&claimed.manifest, &claimed.facts).len(), 3);
    let json = manifest.to_json().unwrap();
    let files = archive(&[
        ("manifest.json", json.as_bytes()),
        ("image.png", &image(1080, 1920, ImageFormat::Png)),
        ("preview.png", &png),
    ]);
    let facets = ["1080p", "android", "desktop", "portrait"];
    assert_eq!(read(Cursor::new(files), &Limits::default()).unwrap().facts.facets, facets);
    manifest.requires = Some(wprs_format::Requires { features: vec!["sound".into()], ..Default::default() });
    let found = load(&manifest, &png).unwrap();
    assert_eq!(features::warnings(&found.manifest, &found.facts).len(), 1);
    for preview in
        [image(1025, 10, ImageFormat::Png), image(10, 10, ImageFormat::Jpeg), b"not an image".to_vec()]
    {
        let result = load(&manifest, &preview);
        assert!(
            matches!(&result, Err(Error::Invalid { field, .. }) if field == "previews.thumbnail"),
            "{result:?}"
        );
    }
    manifest.previews = Some(Previews { thumbnail: None, animated: Some("loop.gif".into()) });
    let with = |gif: Vec<u8>| {
        read(Cursor::new(package(&manifest, &[("loop.gif", &gif)])), &Limits::default()).map(drop)
    };
    with(gif(3, 100)).unwrap();
    assert!(matches!(with(gif(16, 100)), Err(Error::Invalid { field, .. }) if field == "previews.animated"));
}

#[test]
fn upgrade_converts_a_version_one_source() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    source(&src, &Manifest { preview: Some("preview.png".into()), ..v1(Kind::Scene) });
    fs::write(src.join("manifest.json.minisig"), "stale").unwrap();
    let content = Content { rating: Rating::General, descriptors: Vec::new() };
    let origin = Origin::default();
    let (upgraded, _) = upgrade(&src, content.clone(), false, origin.clone()).unwrap();
    assert_eq!(
        (upgraded.inputs.as_slice(), upgraded.features()),
        (&[Input::Audio][..], &["inputs.audio".to_string()][..])
    );
    assert_eq!(
        (upgraded.thumbnail().map(String::as_str), upgraded.preview.as_ref()),
        (Some("preview.png"), None)
    );
    assert!(!src.join("manifest.json.minisig").exists());
    build(&src, tmp.path().join("up.wpak"), &Limits::default()).unwrap();
    let again = upgrade(&src, content, false, origin);
    assert!(matches!(&again, Err(Error::Invalid { field, .. }) if field == "schema"), "{again:?}");
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        match entry.path().is_dir() {
            true => copy(&entry.path(), &to.join(entry.file_name())),
            false => drop(fs::copy(entry.path(), to.join(entry.file_name())).unwrap()),
        }
    }
}

#[test]
fn released_example_scenes_upgrade_into_valid_packages() {
    let released: BTreeMap<String, Value> =
        serde_json::from_str(include_str!("fixture/released.json")).unwrap();
    let (tmp, root) =
        (tempfile::tempdir().unwrap(), Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenes"));
    for (name, manifest) in released {
        let dir = tmp.path().join(&name);
        copy(&root.join(&name), &dir);
        fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
        let mut scene: Value = serde_json::from_slice(&fs::read(dir.join("scene.json")).unwrap()).unwrap();
        let known = |layer: &Value| {
            layer["image"]["prop"].as_str().is_none_or(|p| manifest["properties"].get(p).is_some())
        };
        scene["layers"].as_array_mut().unwrap().retain(known);
        fs::write(dir.join("scene.json"), scene.to_string()).unwrap();
        let content = Content { rating: Rating::General, descriptors: Vec::new() };
        let (upgraded, renames) = upgrade(&dir, content, false, Origin::by(&[])).unwrap();
        let localized = manifest.to_string().contains(r#""es":"#);
        assert_eq!((upgraded.default_locale.is_some(), renames.len()), (localized, 0), "{name}");
        assert_eq!(upgraded.thumbnail().map(String::as_str), Some("preview.jpg"));
        build(&dir, tmp.path().join(format!("{name}.wpak")), &Limits::default()).unwrap();
    }
}

#[test]
fn upgrades_rename_hyphenated_properties_and_find_the_default_language() {
    let label = |es: &str, de: &str| json!({"es": es, "de": de});
    let mut manifest = json!({"schema": "wallpaper-rs.wpak/1", "id": "org.example.old", "version": "1.0.0",
        "title": "Old", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "properties": {
            "snow-speed": {"type": "slider", "min": 0, "max": 9, "step": 1, "default": 3, "label": label("Velocidad", "Tempo")},
            "snow_speed": {"type": "bool", "default": true, "visible_if": {"snow-speed": 3}, "label": label("Nieve", "Schnee")}},
        "presets": [{"id": "fast", "values": {"snow-speed": 9}}]});
    let mut scene = json!({"layers": [{"opacity": {"prop": "snow-speed", "scale": 0.1}}],
        "timelines": [{"id": "intro", "trigger": "prop:snow-speed"}]});
    let renames = upgrade_json(&mut manifest, Some(&mut scene)).unwrap();
    assert_eq!(renames, BTreeMap::from([("snow-speed".to_string(), "snow_speed_2".to_string())]));
    assert_eq!(manifest["properties"]["snow_speed"]["visible_if"], json!({"snow_speed_2": 3}));
    assert_eq!(manifest["presets"][0]["values"], json!({"snow_speed_2": 9}));
    assert_eq!(scene["layers"][0]["opacity"], json!({"prop": "snow_speed_2", "scale": 0.1}));
    assert_eq!(
        (&scene["timelines"][0]["trigger"], &manifest["default_locale"]),
        (&json!("prop:snow_speed_2"), &json!("de"))
    );
    let upgraded: Manifest = serde_json::from_value(manifest.clone()).unwrap();
    let missing: Vec<String> = upgraded.missing().iter().map(ToString::to_string).collect();
    assert_eq!(missing.len(), 3, "{missing:?}");
    assert!(missing[0].contains("content") && missing[2].contains("origin"));
    assert!(upgrade_json(&mut manifest, None).unwrap_err().to_string().contains("already"));
}

#[test]
fn renames_reach_conditions_of_both_spellings() {
    let mut manifest = json!({"properties": {"mood": {"type": "combo", "options": ["a", "b"], "default": "a"},
        "moody": {"type": "bool", "default": false},
        "tint": {"type": "color", "default": "#ffffff", "enabled_if": {"mood": "b"}, "visible_if": "mood == 'mood' || !moody"}}});
    rename_json(&mut manifest, None, &BTreeMap::from([("mood".to_string(), "feel".to_string())]));
    let tint = &manifest["properties"]["tint"];
    assert_eq!(tint["enabled_if"], json!({"feel": "b"}));
    assert_eq!(tint["visible_if"], "feel == 'mood' || !moody");
    assert!(manifest["properties"].get("feel").is_some() && manifest["properties"].get("mood").is_none());
}

#[test]
fn inputs_count_every_property_value_and_clicks() {
    let scene = |effects: &str, props: &str| {
        let scene = format!(
            r##"{{"schema": "wallpaper-rs.scene/1", "canvas": {{"size": [64, 32]}},
            "layers": [{{"id": "fill", "type": "solid", "color": "#336699", "effects": [{effects}]}}]}}"##
        );
        let properties = serde_json::from_str(props).unwrap();
        let manifest = Manifest { properties, ..v2(Kind::Scene) };
        let parsed = wprs_format::Scene::load(&scene, &manifest.properties).unwrap();
        (features::facts(&manifest, Some(&parsed), None), manifest)
    };
    let gated = r#"{"effect": "chromatic_aberration", "enabled": {"prop": "react"}, "params": {"amount": {"audio": "bass"}}}"#;
    let (facts, manifest) = scene(gated, r#"{"react": {"type": "bool", "default": false}}"#);
    assert_eq!(facts.inputs.iter().copied().collect::<Vec<_>>(), [Input::Audio]);
    let refused = features::check(&manifest, &facts).unwrap_err().to_string();
    assert!(refused.contains("audio at /layers/0/effects/0 of scene.json"), "{refused}");
    let ripple = r#"{"effect": "water_ripple", "params": {"click": {"prop": "touch"}}}"#;
    let (facts, _) =
        scene(ripple, r#"{"touch": {"type": "slider", "min": 0, "max": 8, "step": 1, "default": 0}}"#);
    assert_eq!(facts.inputs.iter().copied().collect::<Vec<_>>(), [Input::Pointer, Input::Clicks]);
    let (quiet, _) = scene(r#"{"effect": "water_ripple", "params": {"click": 0}}"#, "{}");
    assert_eq!(quiet.inputs.iter().copied().collect::<Vec<_>>(), [Input::Pointer]);
}
