use super::super::SceneSpec;
use super::super::power::Power;
use super::SceneState;
use crate::scenes::ScenePack;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_render::Renderer;
use wprs_scene::{SCHEMA, Scene, Tier};

fn halves() -> Arc<ScenePack> {
    let solid = |id: &str, color: &str, x: f32| json!({ "id": id, "type": "solid", "color": color, "size": [32, 16], "transform": { "position": [x, 8] } });
    let timelines = [json!({ "id": "t", "duration": 100, "loop": "repeat" })];
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 16], "background": "#000000", "blend_space": "srgb" },
        "timelines": timelines, "layers": [solid("left", "#ff0000", 16.0), solid("right", "#0000ff", 48.0)] });
    Arc::new(ScenePack::bare(Scene::from_json(&text.to_string()).unwrap(), Default::default(), Vec::new()))
}

fn part(pack: &Arc<ScenePack>, span: [f32; 4]) -> SceneState {
    let spec = SceneSpec {
        values: Default::default(),
        pack: pack.clone(),
        extra: Vec::new(),
        clips: Vec::new(),
        hold: None,
        moving: false,
        quality: None,
        span: Some(span),
    };
    SceneState::new(spec, Power::new(Tier::T3, false, 60))
}

#[test]
fn each_screen_draws_its_own_part_of_one_canvas_on_one_clock() {
    let Ok(renderer) = Renderer::shared() else {
        return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required");
    };
    let pack = halves();
    let (mut left, mut right) = (part(&pack, [0.0, 0.0, 32.0, 16.0]), part(&pack, [32.0, 0.0, 32.0, 16.0]));
    left.share(None);
    let clock = left.shared.clone();
    right.share(clock);
    assert!(Arc::ptr_eq(left.shared.as_ref().unwrap(), right.shared.as_ref().unwrap()));
    let (power, start) = (Power::new(Tier::T3, false, 60), Instant::now());
    for at in [start, start + Duration::from_millis(250), start + Duration::from_millis(400)] {
        left.render(renderer, ((16, 8), 60), power, &HashMap::new(), true, at).unwrap();
        right.render(renderer, ((16, 8), 60), power, &HashMap::new(), true, at).unwrap();
        assert!((left.time() - right.time()).abs() < 1e-9, "{} {}", left.time(), right.time());
    }
    assert!((left.time() - 0.4).abs() < 1e-6, "{}", left.time());
    let read = |s: &SceneState| renderer.read_image(s.image().unwrap()).unwrap();
    let (l, r) = (read(&left), read(&right));
    let row =
        |i: &image::RgbaImage| (0..i.width()).map(|x| i.get_pixel(x, i.height() / 2).0).collect::<Vec<_>>();
    assert!(l.pixels().all(|p| p.0[0] > 200 && p.0[2] < 40), "{:?} {:?}", l.dimensions(), row(&l));
    assert!(r.pixels().all(|p| p.0[2] > 200 && p.0[0] < 40), "{:?}", r.get_pixel(0, 4));
    let mut late = part(&pack, [16.0, 0.0, 32.0, 16.0]);
    late.share(left.shared.clone());
    assert!((late.time() - left.time()).abs() < 1e-6, "a screen that joins starts at the shared time");
    late.spec.span = None;
    late.share(None);
    assert!(late.shared.is_none());
}
