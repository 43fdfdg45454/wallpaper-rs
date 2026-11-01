use super::super::super::SceneSpec;
use super::super::super::power::Power;
use super::*;
use crate::scenes::ScenePack;
use serde_json::json;
use wprs_scene::{Scene, Tier};

fn state(folder: Option<&std::path::Path>) -> SceneState {
    let scene = json!({ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [16, 9] },
        "scripts": [{ "path": "main.js", "uses": ["folders"] }],
        "layers": [{ "id": "photo", "type": "solid", "color": "#ffffff" }] });
    let scene = Scene::from_json(&scene.to_string()).unwrap();
    let properties = serde_json::from_value(json!({ "album": { "type": "folder", "mode": "all" } })).unwrap();
    let pack = Arc::new(ScenePack::bare(scene, properties, Vec::new()));
    let chosen = folder.map(|f| json!({ "album": f }));
    let values = serde_json::from_value(chosen.unwrap_or(json!({}))).unwrap();
    let spec = SceneSpec { values, pack, extra: Vec::new(), clips: Vec::new(), hold: None, moving: false };
    SceneState::new(spec, Power::new(Tier::T3, false, 0))
}

fn listed(state: &mut SceneState, at: Instant) -> bool {
    (0..200).any(|_| state.listing(at) || (std::thread::sleep(Duration::from_millis(5)), false).1)
}

#[test]
fn a_still_scene_looks_at_its_folder_off_the_compositor_and_wakes_for_it() {
    let dir = tempfile::tempdir().unwrap();
    image::RgbaImage::new(2, 2).save(dir.path().join("a.png")).unwrap();
    let (mut state, start) = (state(Some(dir.path())), Instant::now());
    assert!(!state.listing(start) && state.listing_wake(start) == Wake::At(SOON.as_secs_f64()));
    assert!(listed(&mut state, start), "the first listing never arrived");
    assert_eq!(state.listed.text.matches("a.png").count(), 1);
    assert_eq!(state.listing_wake(start), Wake::At(EVERY.as_secs_f64()));
    assert!(!state.listing(start + Duration::from_secs(1)), "a second look before 2 s");
    image::RgbaImage::new(2, 2).save(dir.path().join("b.png")).unwrap();
    let later = start + EVERY;
    assert!(listed(&mut state, later), "a new file was not seen");
    assert!(state.listed.text.contains("b.png"));
    let again = later + EVERY;
    assert!(!listed(&mut state, again), "an unchanged folder counted as a change");
    let mut unset = self::state(None);
    assert!(unset.listing(start) && unset.listed.text == r#"{"album":[]}"#);
    assert_eq!(unset.listing_wake(start + EVERY), Wake::Idle, "no folder chosen, nothing to look at");
}
