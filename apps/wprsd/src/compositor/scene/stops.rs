use super::*;
use crate::scenes::ScenePack;
use serde_json::json;
use std::sync::Arc;
use wprs_core::QualityChoice;
use wprs_scene::{SCHEMA, Scene, Tier};

fn heavy() -> SceneSpec {
    let blur = |i: usize| json!({ "id": format!("b{i}"), "type": "adjustment", "effects": [{ "effect": "blur", "params": { "radius": 30 } }] });
    let layers: Vec<_> = (0..120).map(blur).collect();
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [960, 540] }, "clock": { "poster_time": 2.0 }, "layers": layers });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let pack = Arc::new(ScenePack::bare(scene, Default::default(), Vec::new()));
    let (values, extra, clips) = (pack.props.values(&[]), Vec::new(), Vec::new());
    SceneSpec { values, pack, extra, clips, hold: None, moving: true, quality: None, span: None }
}

#[test]
fn a_fixed_level_never_freezes_a_scene_too_heavy_for_the_device_and_automatic_still_does() {
    let auto = Power::new(Tier::T0, true, 60);
    let state = SceneState::new(heavy(), auto);
    assert!(state.still && state.runtime.time() == 2.0, "automatic left the heavy scene moving");
    let level = state.quality();
    assert_eq!((level.reason, level.automatic, level.cost), ("device", "stop", 0.0));
    for choice in ["q0", "q2", "min-q1"] {
        let chosen = Power { choice: QualityChoice::parse(choice).unwrap(), saver: true, ..auto };
        let mut state = SceneState::new(heavy(), chosen);
        assert!(!state.still && state.needs(state.quality) == state.uses, "{choice} froze the scene");
        state.level(chosen);
        let level = state.quality();
        assert_eq!((level.reason, level.automatic), ("user", "stop"), "{choice}");
        assert!(level.cost > level.budget && level.budget == Tier::T0.budget(), "{choice}");
        state.level(auto);
        assert!(state.still && state.runtime.time() == 2.0, "switching to automatic did not stop it");
        state.level(chosen);
        assert!(!state.still, "{choice} did not play it again");
    }
    let own = SceneSpec { quality: Some(QualityChoice::Fixed(1)), ..heavy() };
    let mut state = SceneState::new(own, auto);
    state.level(auto);
    assert!(!state.still && state.quality().quality == "q1", "the own level of the output was not kept");
}
