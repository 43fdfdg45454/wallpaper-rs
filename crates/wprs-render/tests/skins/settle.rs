use super::kit::{SIDE, mesh, picture, renderer, scene, sketch};
use serde_json::json;
use std::sync::Arc;
use wprs_render::{Pixels, SceneAssets};
use wprs_scene::{Quality, Runtime, Values, Wake};

#[test]
fn a_settled_skeleton_redraws_nothing_and_sleeps() {
    let Some(r) = renderer() else { return };
    let turn = json!({ "clips": [{ "id": "turn", "loop": "once", "bones": { "arm": { "rotation": [[0, 0], [0.5, 40]] } } }],
        "animations": [{ "clip": "turn" }] });
    let scene = scene(mesh(turn), sketch(false));
    let assets =
        SceneAssets { images: [("pic".to_string(), Pixels::new(picture()))].into(), ..Default::default() };
    let mut view = r.scene_view(&scene, &assets, &Values::new()).unwrap();
    let mut runtime = Runtime::new(Arc::new(scene), Values::new());
    let size = (SIDE, SIDE);
    let mut step = |runtime: &mut Runtime| {
        runtime.advance(0.2);
        let frame = runtime.frame(view.view(size), Quality::Q3);
        r.render_scene(&mut view, runtime, &frame, size, Quality::Q3, &[]).unwrap();
        (frame.wake, view.damage())
    };
    let (wake, damage) = step(&mut runtime);
    assert_eq!(wake, Wake::Frame);
    assert!(damage.is_none_or(|d| d.width > 0));
    (0..3).for_each(|_| _ = step(&mut runtime));
    let (wake, damage) = step(&mut runtime);
    assert_eq!((wake, damage), (Wake::Idle, Some(wprs_core::Region::default())));
}
