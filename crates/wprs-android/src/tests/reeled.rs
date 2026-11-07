use super::*;
use crate::reel;

fn built() -> (String, std::path::PathBuf) {
    let source = tempfile::tempdir().unwrap();
    fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), source.path().join("clip.webm")).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.reel", "version": "1.0.0",
        "title": "Reel", "license": "CC0-1.0", "kind": "video", "entry": "clip.webm", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" },
        "playback": { "speed": 0.75, "start": 0.5, "end": 1.25, "volume": 0.4, "loop": false, "max_fps": 24 },
        "display": { "zoom": 1.5, "position": [0, -0.5] },
        "presets": [{ "id": "calm", "label": "Calm", "values": { "speed": 0.5, "volume": 20 } }] });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let wpak = source.path().join("reel.wpak");
    wprs_format::build(source.path(), &wpak, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&wpak).unwrap(), "reel.wpak").unwrap().0;
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    (uid, path)
}

pub(super) fn video_settings_play_on_the_phone() {
    let android = wprs_format::Platform::Android;
    assert_eq!(
        wprs_format::features::support(android, "video_properties"),
        wprs_format::features::Support::Runs
    );
    let (uid, path) = built();
    let form = ask(json!({ "op": "scene", "uid": uid, "languages": [] }));
    let keys: Vec<&str> =
        form["fields"].as_array().unwrap().iter().filter_map(|f| f["key"].as_str()).collect();
    assert_eq!(
        (form["video"].clone(), keys),
        (json!(true), vec!["speed", "start", "end", "volume", "zoom", "x", "y"])
    );
    assert_eq!(form["presets"][0]["value"], json!("calm"), "{form}");
    let shown = reel::of(Some(&path));
    let authored = reel::Reel {
        rate: 0.75,
        start: 500,
        end: Some(1250),
        volume: 0.4,
        once: true,
        fps: 24,
        zoom: 1.5,
        pan: [0.0, -0.5],
    };
    assert_eq!(shown, authored);
    assert!(shown.framed() && !shown.direct());
    assert_eq!(
        shown.told(),
        json!({ "rate": 0.75, "start": 500, "end": 1250, "loop": false, "volume": 0.4, "fps": 24 })
    );
    let set =
        |key: &str, value: Value| ask(json!({ "op": "property", "uid": uid, "key": key, "value": value }));
    assert_eq!(set("speed", json!(2)), Value::Null);
    assert_eq!(set("end", json!(0)), Value::Null);
    assert_eq!((reel::of(Some(&path)).rate, reel::of(Some(&path)).end), (2.0, None));
    for (key, bad) in
        [("speed", json!(true)), ("zoom", json!("far")), ("x", json!("left")), ("volume", json!([1]))]
    {
        assert!(set(key, bad.clone())["error"].is_string(), "{key} took {bad}");
    }
    for (key, high) in [("speed", 9), ("zoom", 0), ("volume", 101), ("x", -3)] {
        assert_eq!(set(key, json!(high)), Value::Null);
    }
    let held = reel::of(Some(&path));
    assert_eq!(
        (held.rate, held.zoom, held.volume, held.pan[0]),
        (4.0, 1.0, 1.0, -1.0),
        "values outside are held in range"
    );
    set("x", json!(0));
    assert!(set("color", json!(1))["error"].is_string());
    set("zoom", json!(1));
    set("y", json!(0));
    let plain = reel::of(Some(&path));
    assert!(plain.direct(), "a frame cap plays straight on the screen, which is asked for that rate");
    assert_eq!(plain.cap(), 24);
    assert_eq!(ask(json!({ "op": "preset", "uid": uid, "preset": "calm" })), Value::Null);
    let calm = reel::of(Some(&path));
    assert_eq!((calm.rate, calm.volume), (0.5, 0.2));
    assert_eq!(ask(json!({ "op": "reset", "uid": uid })), Value::Null);
    assert_eq!(reel::of(Some(&path)), authored);
    assert_eq!(reel::of(None), reel::Reel::default());
    assert!(reel::Reel::default().direct() && reel::Reel::default().cap() == 0);
    assert_eq!(reel::Reel::default().told()["fps"], json!(0));
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
