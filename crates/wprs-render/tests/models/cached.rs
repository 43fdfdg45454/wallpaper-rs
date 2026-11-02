use super::*;
use wprs_scene::Wake;

fn still() -> Value {
    json!([{ "id": "cube", "type": "model", "model": "red", "rotation": [20, 30, 0] }])
}

#[test]
fn a_still_model_renders_once_and_the_scene_sleeps() {
    let Some(r) = renderer() else { return };
    let models = BTreeMap::from([("red", cube(ForgeMaterial::new("paint", [0.8, 0.1, 0.1, 1.0])))]);
    let mut shot = shoot(r, scene(json!({}), still(), models), &[]);
    let first = shot.frame(r, 0.0, Quality::Q3);
    for _ in 0..4 {
        assert_eq!(shot.frame(r, 0.1, Quality::Q3), first);
    }
    assert_eq!(shot.view.model_renders(), 1);
    let frame = shot.runtime.frame(shot.view.view((SIDE, SIDE)), Quality::Q3);
    assert_eq!(frame.wake, Wake::Idle);
    assert_eq!(shot.view.damage(), Some(wprs_core::Region::default()));
}

#[test]
fn a_3d_group_draws_again_only_while_something_in_it_moves() {
    let Some(r) = renderer() else { return };
    let models = BTreeMap::from([("red", cube(unlit("paint", [1.0, 0.0, 0.0, 1.0])))]);
    let spin = json!([{ "id": "world", "type": "group", "space": "3d", "layers": [
        { "id": "cube", "type": "model", "model": "red",
          "rotation": { "keys": [[0, [0, 0, 0]], [0.3, [0, 90, 0]]] } }] }]);
    let mut shot = shoot(r, scene(json!({}), spin, models), &[]);
    let mut seen = Vec::new();
    for _ in 0..6 {
        shot.frame(r, 0.1, Quality::Q3);
        seen.push(shot.view.model_renders());
    }
    assert!(seen[2] > seen[0], "{seen:?}");
    assert_eq!(seen[4], seen[5], "{seen:?}");
    let frame = shot.runtime.frame(shot.view.view((SIDE, SIDE)), Quality::Q3);
    assert_eq!(frame.wake, Wake::Idle);
}

#[test]
fn the_lowest_quality_still_shows_the_model_without_reflections_of_its_picture() {
    let Some(r) = renderer() else { return };
    let models = BTreeMap::from([("red", cube(unlit("paint", [1.0, 0.0, 0.0, 1.0])))]);
    let px = render(r, scene(json!({}), still(), models), Quality::Q0);
    assert!(rgb(&px, 32, 32)[0] > 200, "{:?}", rgb(&px, 32, 32));
}

#[test]
fn models_and_reflections_that_leave_the_scene_free_their_gpu_memory() {
    let Some(r) = renderer() else { return };
    let models = BTreeMap::from([
        ("red", cube(unlit("paint", [1.0, 0.0, 0.0, 1.0]))),
        ("blue", cube(unlit("paint", [0.0, 0.0, 1.0, 1.0]))),
    ]);
    let lit = json!([{ "id": "cube", "type": "model", "model": "red", "env": "sky" }]);
    let first = scene(json!({ "assets": { "sky": { "path": "sky.png" } } }), lit, models.clone());
    let sky = Pixels::new(RgbaImage::from_pixel(8, 8, Rgba([0, 120, 255, 255])));
    let mut shot = shoot(r, first, &[("sky", sky)]);
    shot.frame(r, 0.0, Quality::Q3);
    assert_eq!(shot.view.held_models(), (1, 1));
    let second = scene(json!({}), json!([{ "id": "cube", "type": "model", "model": "blue" }]), models);
    r.refresh_scene(&mut shot.view, (&second, &wprs_scene::Values::new()), &SceneAssets::default()).unwrap();
    shot.runtime = Runtime::new(Arc::new(second), wprs_scene::Values::new());
    let px = shot.frame(r, 0.0, Quality::Q3);
    assert!(near(rgb(&px, 32, 32), [0, 0, 255], 3), "{:?}", rgb(&px, 32, 32));
    assert_eq!(shot.view.held_models(), (1, 0), "only the blue cube stays on the GPU");
}
