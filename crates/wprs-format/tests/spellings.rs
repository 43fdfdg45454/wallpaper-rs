mod common;

use common::{image, manifest as v1, v2};
use image::ImageFormat;
use serde_json::json;
use std::fs;
use wprs_format::{Error, Kind, Manifest, features, inspect};

fn with(mut manifest: Manifest, properties: serde_json::Value) -> Manifest {
    manifest.properties = serde_json::from_value(properties).unwrap();
    manifest
}

fn field(result: Result<(), Error>) -> String {
    match result {
        Err(Error::Invalid { field, .. }) => field,
        other => panic!("accepted: {other:?}"),
    }
}

fn written(manifest: &Manifest, property: serde_json::Value) -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().unwrap();
    let mut json = serde_json::to_value(manifest).unwrap();
    json["properties"] = json!({ "p": property });
    fs::write(tmp.path().join("image.png"), image(64, 32, ImageFormat::Png)).unwrap();
    for preview in ["preview.png", "preview.webp"] {
        fs::write(tmp.path().join(preview), image(32, 16, ImageFormat::Png)).unwrap();
    }
    fs::write(tmp.path().join("manifest.json"), json.to_string()).unwrap();
    (tmp, json.to_string())
}

#[test]
fn spellings_older_engines_cannot_read_need_the_second_version() {
    let slider = json!({ "type": "slider", "min": 0, "max": 1, "step": 0.1, "default": 0.5 });
    let with_key = |base: &serde_json::Value, key: &str, value: serde_json::Value| {
        let mut p = base.clone();
        p[key] = value;
        p
    };
    let unreadable = [
        json!({ "type": "text", "label": "City" }),
        json!({ "type": "file", "label": "Photo" }),
        with_key(&slider, "order", json!(2.0)),
        json!({ "type": "group", "collapsed": false }),
        json!({ "type": "text", "default": "", "secret": false }),
        json!({ "type": "bool", "default": true, "enabled_if": {} }),
        with_key(&slider, "editable", json!(false)),
    ];
    for property in unreadable {
        assert!(!wprs_scene::legacy(&property), "{property}");
        let (_v1, raw) = written(&v1(Kind::Image), property.clone());
        assert_eq!(field(Manifest::from_json(raw.as_bytes()).map(drop)), "properties.p", "{property}");
        let (dir, raw) = written(&v2(Kind::Image), property.clone());
        let parsed = Manifest::from_json(raw.as_bytes()).unwrap();
        assert!(parsed.properties["p"].features().is_empty(), "{property}");
        let (manifest, facts) = inspect(dir.path()).unwrap();
        assert!(facts.features.contains("properties2"), "{property}");
        assert_eq!(field(features::check(&manifest, &facts)), "requires.features");
    }
    for property in
        [json!({ "type": "text", "label": "City", "default": "" }), with_key(&slider, "order", json!(2))]
    {
        assert!(wprs_scene::legacy(&property), "{property}");
        let (dir, raw) = written(&v1(Kind::Image), property);
        Manifest::from_json(raw.as_bytes()).unwrap().check().unwrap();
        assert!(!inspect(dir.path()).unwrap().1.features.contains("properties2"));
    }
}

#[test]
fn version_1_files_that_accept_videos_still_open_as_images() {
    let photo = json!({ "type": "file", "label": "Photo", "accept": ["image", "video"] });
    let (_dir, raw) = written(&v1(Kind::Image), photo.clone());
    let parsed = Manifest::from_json(raw.as_bytes()).unwrap();
    parsed.check().unwrap();
    let p = &parsed.properties["p"];
    assert!(p.accepts("image") && !p.accepts("video") && p.features().is_empty());
    with(v1(Kind::Scene), json!({ "p": photo })).check().unwrap();
}
