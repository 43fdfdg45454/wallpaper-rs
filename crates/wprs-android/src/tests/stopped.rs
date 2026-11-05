use super::*;
use crate::stage::report;

fn heavy() -> (String, std::path::PathBuf) {
    let source = tempfile::tempdir().unwrap();
    let blur = |i: usize| json!({ "id": format!("b{i}"), "type": "adjustment", "effects": [{ "effect": "blur", "params": { "radius": 30 } }] });
    let fade = json!({ "id": "fade", "type": "solid", "color": "#ffffff",
        "opacity": { "timeline": "fade", "keys": [[0, 0], [1, 1]] } });
    let layers: Vec<_> = std::iter::once(fade).chain((0..120).map(blur)).collect();
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [960, 540], "background": "#203040" },
        "clock": { "poster_time": 0.5 }, "layers": layers,
        "timelines": [{ "id": "fade", "duration": 1.0, "loop": "repeat" }] });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.heavy", "version": "1.0.0",
        "title": "Heavy", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json",
        "content": { "rating": "general" }, "photosensitive": false, "origin": { "kind": "original" } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("heavy.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&built).unwrap(), "heavy.wpak").unwrap().0;
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    (uid, path)
}

pub(super) fn only_automatic_stops_heavy_scenes() {
    let (uid, path) = heavy();
    let power =
        power::Power { charging: false, saver: true, low: false, thermal: 0, headroom: 0.0, still: false };
    let Some(r) = renderer() else {
        return assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
    };
    let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
    let mut shoot = |n: i64| {
        let shot =
            stage.render(r, ((64, 32), n * 100_000_000, 30), (power, wprs_scene::Tier::T0), &[], false);
        shot.unwrap().fps
    };
    assert_eq!(shoot(0), 0, "automatic played a scene too heavy for the phone");
    assert_eq!((report::automatic(), report::over()), ("still", None));
    for choice in ["q0", "q1", "min-q1"] {
        assert_eq!(settings::setting("playback.quality", Some(choice)).unwrap(), choice);
        assert_eq!(report::automatic(), "stop", "{choice} kept warning of the old choice before a frame");
        assert!(report::over().is_some(), "{choice} hid its cost before a frame");
        assert!(shoot(1) > 0, "{choice} kept the scene still");
        let [cost, budget] = report::over().expect(choice);
        assert!(cost > budget && budget == wprs_scene::Tier::T0.budget(), "{choice} {cost} {budget}");
        assert_eq!(report::automatic(), "stop", "{choice}");
    }
    assert_eq!(settings::setting("playback.quality", Some("auto")).unwrap(), "auto");
    assert_eq!((report::automatic(), report::over()), ("still", None), "auto kept the old cost warning");
    let mut preview = Stage::new(uid.clone(), pack::load(&path).unwrap());
    preview.reports = false;
    assert_eq!(settings::setting("playback.quality", Some("q0")).unwrap(), "q0");
    let at = ((64, 32), 0, 30);
    let (level, left) = (report::level(), report::left_out());
    assert!(preview.render(r, at, (power, wprs_scene::Tier::T3), &[], false).unwrap().fps > 0);
    assert_eq!(report::level(), level, "a preview wrote the wallpaper's level");
    assert_eq!(report::left_out(), left, "a preview wrote the wallpaper's left-out effects");
    assert_eq!(level.map(|[tier, ..]| tier), Some("T0".to_owned()));
    assert_eq!(
        report::over().map(|[_, b]| b),
        Some(wprs_scene::Tier::T0.budget()),
        "a preview wrote the report"
    );
    assert_eq!(settings::setting("playback.quality", Some("auto")).unwrap(), "auto");
    assert_eq!((shoot(2), report::automatic()), (0, "still"), "automatic did not stop it again");
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
