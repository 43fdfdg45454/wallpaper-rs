use super::{ask, renderer};
use crate::stage::Stage;
use crate::{library, pack, power, store, tilt};
use serde_json::{Value, json};
use std::fs::{self, File};

pub(super) fn scenes_follow_the_phone() {
    let source = tempfile::tempdir().unwrap();
    let layers = json!([{ "id": "sky", "type": "solid", "color": "#336699" },
        { "id": "hill", "type": "solid", "color": "#88aa44", "size": [32, 16], "parallax": 0.5 }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] }, "layers": layers });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.hills", "version": "1.0.0",
        "title": "Hills", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json",
        "content": { "rating": "general" }, "photosensitive": false, "origin": { "kind": "original" }, "inputs": ["pointer"] });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("hills.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&built).unwrap(), "hills.wpak").unwrap().0;
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    let pack = pack::load(&path).unwrap();
    assert!(pack.scene.tilts(), "a scene with parallax layers follows the phone without asking for it");
    ask(json!({ "op": "pick", "choice": { "item": uid } }));
    assert!(tilt::wanted(true, false));
    let mut stage = Stage::new(uid.clone(), pack);
    assert!(!stage.offset(0.5) && stage.offset(0.0) && !stage.offset(0.0));
    let charging =
        power::Power { charging: true, saver: false, low: false, thermal: 0, headroom: 0.0, still: false };
    if let Some(r) = renderer() {
        let draw = |stage: &mut Stage, nanos: i64, force: bool| {
            let power = (charging, wprs_scene::Tier::T3);
            stage.render(r, ((64, 32), nanos, 60), power, &[], force).unwrap().image.is_some()
        };
        assert!(draw(&mut stage, 0, true));
        assert!(!draw(&mut stage, 50_000_000, false), "a still phone draws nothing more");
        stage.offset(1.0);
        assert!(draw(&mut stage, 100_000_000, false), "a new launcher page moves the layers");
        assert!(stage.runtime().parallax()[0] < 0.0);
    }
    ask(json!({ "op": "pick", "choice": null }));
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
