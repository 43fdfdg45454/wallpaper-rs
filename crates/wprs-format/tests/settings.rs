mod common;

use common::{image, manifest as v1, v2};
use image::ImageFormat;
use serde_json::json;
use std::fs;
use wprs_format::{Display, Error, Kind, Manifest, Playback, Requires, VIDEO_PROPERTIES, features, inspect};

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

fn newer() -> serde_json::Value {
    json!({ "mode": { "type": "combo", "default": 2, "options": [{ "value": 1, "label": "One" }, { "value": 2, "label": "Two" }] },
            "again": { "type": "button", "label": "Again", "visible_if": "mode == 2" } })
}

#[test]
fn newer_kinds_of_settings_need_the_second_version_and_their_feature() {
    assert_eq!(field(with(v1(Kind::Scene), newer()).check()), "properties");
    let manifest = with(v2(Kind::Scene), newer());
    manifest.check().unwrap();
    let facts = features::facts(&manifest, None, None);
    assert!(facts.features.contains("properties2") && facts.facets.contains(&"customizable".into()));
    assert_eq!(field(features::check(&manifest, &facts)), "requires.features");
    let listed = Manifest {
        requires: Some(Requires { features: vec!["properties2".into()], ..Default::default() }),
        ..manifest
    };
    features::check(&listed, &facts).unwrap();
    let json = listed.to_json().unwrap();
    assert_eq!(Manifest::from_json(json.as_bytes()).unwrap(), listed);
    let older = json.replace("properties2", "properties9");
    assert!(matches!(Manifest::from_json(older.as_bytes()), Err(Error::Newer(_))));
}

#[test]
fn files_and_folders_accept_images_and_videos_only() {
    let file = |accept: serde_json::Value| {
        with(v2(Kind::Scene), json!({ "photo": { "type": "file", "accept": accept } }))
    };
    file(json!(["image", "video"])).check().unwrap();
    assert_eq!(field(file(json!(["png"])).check()), "properties");
    with(v1(Kind::Scene), json!({ "photo": { "type": "file", "accept": ["png"] } })).check().unwrap();
    let folder =
        with(v2(Kind::Scene), json!({ "photos": { "type": "folder", "accept": ["image", "video"] } }));
    let facts = features::facts(&folder, None, None);
    assert!(facts.features.contains("folder") && facts.features.contains("user_videos"));
    assert_eq!(features::lacks("android", &facts.features), ["folder", "user_videos"]);
}

#[test]
fn video_packages_set_how_their_video_plays() {
    let playback = Playback { speed: Some(1.5), start: Some(2.0), end: Some(8.0), volume: Some(0.4) };
    let display = Display { fit: Some(wprs_core::Fit::Fit), zoom: Some(2.0), position: Some([0.5, -0.25]) };
    let video =
        Manifest { playback: Some(playback.clone()), display: Some(display.clone()), ..v2(Kind::Video) };
    video.check().unwrap();
    assert!(features::facts(&video, None, None).features.contains("video_properties"));
    assert_eq!(field(Manifest { playback: Some(playback.clone()), ..v2(Kind::Scene) }.check()), "playback");
    assert_eq!(field(Manifest { playback: Some(playback.clone()), ..v1(Kind::Video) }.check()), "schema");
    let fast = Playback { speed: Some(5.0), ..playback.clone() };
    assert_eq!(field(Manifest { playback: Some(fast), ..v2(Kind::Video) }.check()), "playback");
    let backwards = Playback { end: Some(1.0), ..playback };
    assert_eq!(field(Manifest { playback: Some(backwards), ..v2(Kind::Video) }.check()), "playback");
    let far = Display { position: Some([2.0, 0.0]), ..display };
    assert_eq!(field(Manifest { display: Some(far), ..v2(Kind::Video) }.check()), "display");
    let labels = ["Speed", "Start", "Stop at", "Volume", "Zoom", "X", "Y", "0 is the end"].map(String::from);
    let props = video.video_properties(&labels, Some(12.0));
    assert_eq!(props.keys().collect::<Vec<_>>(), {
        let mut ids = VIDEO_PROPERTIES.to_vec();
        ids.sort();
        ids
    });
    let default = |id: &str| props[id].default_value();
    assert_eq!(
        [
            default("speed"),
            default("start"),
            default("end"),
            default("volume"),
            default("zoom"),
            default("x"),
            default("y")
        ],
        [json!(1.5), json!(2.0), json!(8.0), json!(40), json!(2.0), json!(0.5), json!(-0.25)]
    );
    assert!(props.values().all(|p| p.check().is_ok() && p.features() == ["properties2"]));
    assert_eq!(props["start"].value(&json!(30.0)), Ok(json!(11.5)));
    assert_eq!(props["end"].value(&json!(30.0)), Ok(json!(12.0)));
}

#[test]
fn video_presets_set_the_playback_settings() {
    let presets = |values: serde_json::Value| {
        serde_json::from_value(json!([{ "id": "slow", "label": "Slow", "values": values }])).unwrap()
    };
    let video = Manifest { presets: presets(json!({ "speed": 0.5, "zoom": 1.5 })), ..v2(Kind::Video) };
    video.check().unwrap();
    let wrong = Manifest { presets: presets(json!({ "speed": 9, "tint": "#ff0000" })), ..v2(Kind::Video) };
    assert_eq!(field(wrong.check()), "presets.0.values.tint");
}

#[test]
fn image_variants_of_another_shape_are_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = with(
        v2(Kind::Scene),
        json!({ "sky": { "type": "combo", "default": "day", "options": ["day", "night"] } }),
    );
    let scene = json!({ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
        "assets": { "day": { "path": "day.png" }, "night": { "path": "night.png" } },
        "layers": [{ "id": "sky", "type": "image", "image": { "prop": "sky", "map": { "day": "day", "night": "night" } } },
                   { "id": "veil", "type": "solid", "color": "#000000", "size": [64, 32],
                     "mask": { "texture": { "prop": "sky", "map": { "day": "day", "night": "night" } } } }],
        "particles": { "dust": { "capacity": 10, "emitters": [{ "shape": { "point": [32, 16] }, "rate": 5 }],
                                "init": { "life": [1, 2] }, "render": { "texture": { "prop": "sky", "map": { "day": "day", "night": "night" } } } } } });
    let write = |night: Vec<u8>| {
        fs::write(tmp.path().join("manifest.json"), manifest.to_json().unwrap()).unwrap();
        fs::write(tmp.path().join("scene.json"), scene.to_string()).unwrap();
        [
            ("preview.png", image(32, 16, ImageFormat::Png)),
            ("day.png", image(64, 32, ImageFormat::Png)),
            ("night.png", night),
        ]
        .iter()
        .for_each(|(name, data)| fs::write(tmp.path().join(name), data).unwrap());
        let (manifest, facts) = inspect(tmp.path()).unwrap();
        features::warnings(&manifest, &facts).into_iter().filter(|w| w.contains("proportion")).count()
    };
    assert_eq!(write(image(128, 64, ImageFormat::Png)), 0);
    assert_eq!(write(image(32, 32, ImageFormat::Png)), 3);
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
