use super::*;
use crate::scenes::ScenePack;
use serde_json::json;
use std::sync::Arc;
use wprs_scene::{SCHEMA, Scene, Tier};

fn slowed(lights: bool) -> (Quality, &'static str) {
    let fade = json!({ "id": "fade", "type": "solid", "color": "#ffffff",
        "opacity": { "timeline": "fade", "keys": [[0, 0], [1, 1]] } });
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 36] }, "layers": [fade],
        "timelines": [{ "id": "fade", "duration": 1.0, "loop": "repeat" }] });
    let pack = Arc::new(ScenePack::bare(
        Scene::from_json(&text.to_string()).unwrap(),
        Default::default(),
        Vec::new(),
    ));
    let (values, extra, clips) = (pack.props.values(&[]), Vec::new(), Vec::new());
    let spec = SceneSpec { values, pack, extra, clips, hold: None, moving: true, quality: None, span: None };
    let mains = Power::new(Tier::T3, false, 0);
    let mut state = SceneState::new(spec, mains);
    let start = Instant::now();
    for sample in 1..=200 {
        state.judge((sample, 50.0), mains, 60, start + Duration::from_millis(16 * sample));
    }
    assert_eq!(state.level(mains), Quality::Q2, "slow frames did not lower the scene");
    state.lights = lights;
    for sample in 201..=1200 {
        state.judge((sample, 0.1), mains, 60, start + Duration::from_millis(33 * sample));
    }
    (state.level(mains), state.quality().reason)
}

#[test]
fn small_frames_drawn_only_for_the_covered_lights_never_raise_the_quality() {
    assert_eq!(slowed(false), (Quality::Q3, "full"), "fast frames on screen did not raise it");
    assert_eq!(slowed(true), (Quality::Q2, "slow"), "covered frames for the lights raised it");
}
