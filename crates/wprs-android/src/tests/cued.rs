use super::{ask, renderer};
use crate::stage::Stage;
use crate::{library, pack, store};
use serde_json::{Value, json};
use std::fs::{self, File};

pub(super) fn cued() {
    let source = tempfile::tempdir().unwrap();
    let layers = json!([{ "id": "box", "type": "solid", "color": "#ffffff", "size": [16, 16],
        "opacity": { "timeline": "fade", "keys": [[0, 0], [1, 1]] } }]);
    let events = json!([{ "t": 1, "name": "tock" }, { "t": 3, "name": "go" }]);
    let timelines = json!([{ "id": "clock", "duration": 4, "events": events },
        { "id": "fade", "duration": 1, "trigger": "event:go" },
        { "id": "wave", "duration": 2, "loop": "repeat", "wrap": true, "autoplay": false }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] }, "layers": layers,
        "timelines": timelines });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.cued", "version": "1.0.0", "title": "Cued",
        "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" },
        "requires": { "features": ["timeline_events", "timeline_wrap"] } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("cued.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&built).unwrap(), "cued.wpak").unwrap().0;
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
    let power = crate::power::Power {
        charging: true,
        saver: false,
        low: false,
        thermal: 0,
        headroom: 0.0,
        still: false,
    };
    if let Some(r) = renderer() {
        let mut draw = |nanos: i64| {
            let shot =
                stage.render(r, ((64, 32), nanos, 60), (power, wprs_scene::Tier::T3), &[], true).unwrap();
            (shot.fps, shot.delay)
        };
        assert_eq!(draw(0), (0, 3_000), "the phone sleeps past events nothing hears");
        assert!(draw(3_050_000_000).0 > 0, "the event starts the fade after a long sleep");
        assert_eq!(draw(4_500_000_000), (0, 0), "and the scene rests once it ends");
    }
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
