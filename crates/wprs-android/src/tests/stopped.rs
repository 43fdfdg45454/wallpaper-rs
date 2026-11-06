use super::*;
use crate::stage::report;
use wprs_core::QualityChoice;

fn warned() -> (String, Option<[f32; 2]>) {
    let shown = report::wallpaper();
    let over = shown["over"].as_array().map(|o| [0, 1].map(|i| o[i].as_f64().unwrap() as f32));
    (shown["auto"].as_str().unwrap().to_owned(), over)
}

fn tier() -> String {
    report::wallpaper()["tier"].as_str().unwrap().to_owned()
}

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

fn stall(stage: &mut Stage) {
    let at = std::time::Instant::now();
    for k in 0..300u32 {
        let when = at + std::time::Duration::from_millis(u64::from(k) * 33);
        stage.pilot().feed(when, 100.0, 33.3, (wprs_scene::Quality::Q0, true, true));
    }
}

pub(super) fn only_automatic_stops_heavy_scenes() {
    let (uid, path) = heavy();
    let power =
        power::Power { charging: false, saver: true, low: false, thermal: 0, headroom: 0.0, still: false };
    let Some(r) = renderer() else {
        return assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
    };
    let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
    let shoot = |stage: &mut Stage, n: i64| {
        let shot =
            stage.render(r, ((64, 32), n * 100_000_000, 30), (power, wprs_scene::Tier::T0), &[], false);
        shot.unwrap().fps
    };
    assert!(shoot(&mut stage, 0) > 0, "automatic stopped a scene before measuring it");
    assert_eq!(warned(), ("".into(), None));
    let (mut n, mut fps) = (1, 0);
    while report::wallpaper()["frame_ms"].is_null() && n < 600 {
        (n, fps) = (n + 1, shoot(&mut stage, n));
    }
    let shown = report::wallpaper();
    assert!(shown["frame_ms"].as_f64().is_some_and(|ms| ms > 0.0), "no frame was measured: {shown}");
    assert!((shown["period_ms"].as_f64().unwrap() - 1000.0 / f64::from(fps)).abs() < 0.01, "{shown}");
    assert!(!matches!(shown["step"].as_str(), None | Some("" | "model")), "{shown}");
    stall(&mut stage);
    assert_eq!(shoot(&mut stage, n), 0, "automatic played a scene measured far too slow at the lowest level");
    assert_eq!(warned(), ("still".into(), None));
    assert_eq!(report::wallpaper()["step"], "still");
    for choice in ["q0", "q1", "min-q1"] {
        assert_eq!(settings::setting("playback.quality", Some(choice)).unwrap(), choice);
        assert_eq!(warned().0, "lower", "{choice} kept warning of the old choice before a frame");
        assert!(warned().1.is_some(), "{choice} hid its cost before a frame");
        assert!(shoot(&mut stage, n + 1) > 0, "{choice} kept the scene still");
        let [cost, budget] = warned().1.expect(choice);
        assert!(cost > budget && budget == wprs_scene::Tier::T0.budget(), "{choice} {cost} {budget}");
        assert_eq!(warned().0, "lower", "{choice}");
    }
    assert_eq!(settings::setting("playback.quality", Some("auto")).unwrap(), "auto");
    assert_eq!(warned(), ("".into(), None), "auto kept the old cost warning");
    let mut preview = Stage::new(uid.clone(), pack::load(&path).unwrap());
    preview.reports = None;
    assert_eq!(settings::setting("playback.quality", Some("q0")).unwrap(), "q0");
    let at = ((64, 32), 0, 30);
    let before = report::wallpaper();
    assert!(preview.render(r, at, (power, wprs_scene::Tier::T3), &[], false).unwrap().fps > 0);
    assert_eq!(report::wallpaper(), before, "a preview wrote the wallpaper's level or left-out effects");
    assert_eq!(before["tier"], "T0");
    assert_eq!(warned().1.map(|[_, b]| b), Some(wprs_scene::Tier::T0.budget()), "a preview wrote the report");
    assert_eq!(settings::setting("playback.quality", Some("auto")).unwrap(), "auto");
    assert!(shoot(&mut stage, n + 2) > 0, "another choice did not measure the scene again");
    stall(&mut stage);
    assert_eq!(
        (shoot(&mut stage, n + 3), warned().0),
        (0, "still".into()),
        "automatic did not stop it again"
    );
    let flips = std::thread::spawn(|| {
        (0..2000).for_each(|i| power::choose([QualityChoice::Fixed(0), QualityChoice::Auto][i % 2]));
    });
    while !flips.is_finished() {
        let told = warned();
        assert!(matches!((told.0.as_str(), told.1), ("still", None) | ("lower", Some(_))), "{told:?}");
    }
    flips.join().unwrap();
    power::choose(QualityChoice::Auto);
    let mut lock = Stage::new(uid.clone(), pack::load(&path).unwrap());
    lock.reports = Some(1);
    lock.render(r, at, (power, wprs_scene::Tier::T3), &[], false).unwrap();
    assert_eq!(tier(), "T3", "the lock screen was not reported");
    report::forget(1);
    assert_eq!(tier(), "T0", "the home scene was forgotten");
    assert_eq!(warned().0, "still", "the home scene's warning went with the lock screen's");
    report::forget(report::HOME);
    let nothing = json!({ "tier": "", "quality": "", "reason": "", "left_out": [], "over": null, "auto": "",
        "frame_ms": null, "period_ms": null, "step": "" });
    assert_eq!(report::wallpaper(), nothing, "an image kept a scene's warning");
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
