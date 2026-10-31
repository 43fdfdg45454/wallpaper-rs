use super::*;

pub(super) fn calmed() {
    let source = tempfile::tempdir().unwrap();
    let opacity = json!({ "timeline": "blink", "keys": [[0, 0], [0.05, 1], [0.1, 0]] });
    let layers = json!([{ "id": "flash", "type": "solid", "color": "#ffffff", "opacity": opacity }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32], "background": "#000000" },
        "clock": { "poster_time": 0.03 }, "layers": layers,
        "timelines": [{ "id": "blink", "duration": 0.1, "loop": "repeat" }] });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.blink", "version": "1.0.0", "title": "Blink",
        "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "content": { "rating": "general" },
        "photosensitive": true, "origin": { "kind": "original" } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("blink.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&built).unwrap(), "blink.wpak").unwrap().0;
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    let power =
        power::Power { charging: true, saver: false, low: false, thermal: 0, headroom: 0.0, still: false };
    let still = power::Power { still: true, ..power };
    if let Some(r) = renderer() {
        let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
        for n in 0..4 {
            let shot =
                stage.render(r, ((64, 32), n * 100_000_000, 60), (still, wprs_scene::Tier::T3), &[], true);
            let shot = shot.unwrap();
            assert!(shot.fps == 0 && (1..=60_000).contains(&shot.delay), "reduced motion kept moving");
        }
        assert_eq!(settings::setting("playback.flash_guard", Some("true")).unwrap(), "true");
        assert!(power::guarded());
        let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
        for n in 0..240 {
            stage
                .render(r, ((64, 32), n * 1_000_000_000 / 60, 60), (power, wprs_scene::Tier::T3), &[], true)
                .unwrap();
        }
        assert_ne!(crate::stage::report::flashes(), "", "the guard let ten flashes a second through");
        settings::setting("playback.flash_guard", Some("false")).unwrap();
        assert!(!power::guarded());
    }
    assert!(power::hears());
    assert_eq!(settings::setting("playback.scene_capture", Some("false")).unwrap(), "false");
    assert!(!power::hears(), "the switch did not stop the scenes from hearing");
    settings::setting("playback.scene_capture", Some("true")).unwrap();
    assert!(power::hears());
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
