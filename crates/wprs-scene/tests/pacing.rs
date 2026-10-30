use serde_json::{Value, json};
use std::sync::Arc;
use wprs_scene::{Budget, Quality, Runtime, SCHEMA, Scene, Tier, Wake};

const VIEW: [f32; 4] = [0.0, 0.0, 1000.0, 1000.0];

fn scene(layers: Value) -> Scene {
    let timelines = [
        json!({ "id": "spin", "duration": 4, "loop": "repeat" }),
        json!({ "id": "move", "duration": 1, "loop": "once" }),
    ];
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [1000, 1000] }, "timelines": timelines, "layers": layers });
    Scene::from_json(&text.to_string()).unwrap()
}

fn solid(id: &str, extra: Value) -> Value {
    let mut layer = json!({ "id": id, "type": "solid", "color": "#ff0000" });
    layer.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    layer
}

#[test]
fn the_cost_counts_the_area_of_each_layer_and_blends_that_copy_the_backdrop() {
    let blurred = |side: u32| {
        let blur = json!([{ "effect": "blur", "params": { "radius": 8 } }]);
        scene(json!([solid("a", json!({ "size": [side, side], "effects": blur }))])).cost(Quality::Q3)
    };
    let (small, full) = (blurred(100), blurred(1000));
    assert!((small - 1.594).abs() < 0.005 && (full - 8.5).abs() < 0.01, "small {small}, full {full}");
    let blended = |blend: &str| scene(json!([solid("a", json!({ "blend": blend }))])).cost(Quality::Q3);
    assert_eq!((blended("normal"), blended("overlay")), (1.5, 4.5));
    let half = scene(json!([solid("a", json!({ "blend": "overlay", "transform": { "scale": [0.5, 1] } }))]));
    assert_eq!(half.cost(Quality::Q3), 3.0);
}

#[test]
fn slow_effects_let_the_scene_run_at_their_lowest_smooth_rate() {
    let waves = solid("a", json!({ "effects": [{ "effect": "waves" }] }));
    let mains = Budget::new(Tier::T3, false, 0);
    let runtime = Runtime::new(Arc::new(scene(json!([waves.clone()]))), Default::default());
    assert_eq!(runtime.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    assert_eq!((runtime.pace(), mains.fps(&runtime, Quality::Q3, 144)), (30, 30));
    let spin = json!({ "transform": { "rotation": { "timeline": "spin", "keys": [[0, 0], [4, 360]] } } });
    let both = Runtime::new(Arc::new(scene(json!([waves, solid("b", spin)]))), Default::default());
    assert_eq!(both.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    assert_eq!(mains.fps(&both, Quality::Q3, 144), 60);
}

#[test]
fn motion_blur_follows_the_layer_and_settles_once_it_stops() {
    let path = json!({ "timeline": "move", "keys": [[0, [0, 500]], [1, [100, 500]]] });
    let effects = json!([{ "effect": "blur_motion", "params": { "follow": true } }]);
    let layer =
        solid("a", json!({ "size": [100, 100], "transform": { "position": path }, "effects": effects }));
    let mut runtime = Runtime::new(Arc::new(scene(json!([layer]))), Default::default());
    let velocity = |runtime: &Runtime| {
        let frame = runtime.frame(VIEW, Quality::Q3);
        (frame.effects[0].velocity, frame.wake)
    };
    assert_eq!(velocity(&runtime).0, [0.0; 2]);
    runtime.advance(0.25);
    let ([vx, vy], wake) = velocity(&runtime);
    assert!((vx - 100.0).abs() < 1.0 && vy.abs() < 1e-3 && wake == Wake::Frame, "{vx} {vy} {wake:?}");
    runtime.advance(0.7);
    assert!(velocity(&runtime).0[0] > 1.0);
    runtime.advance(0.1);
    let ([vx, _], wake) = velocity(&runtime);
    assert!((vx - 50.0).abs() < 0.1 && wake == Wake::Frame, "the last move blurs once more");
    runtime.advance(0.1);
    assert_eq!(velocity(&runtime), ([0.0; 2], Wake::Idle));
}

#[test]
fn reduced_motion_wakes_sprite_sheets_and_paced_effects_once_a_minute() {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [1000, 1000] },
        "assets": { "sheet": { "path": "sheet.png", "atlas": [1, 4], "fps": 8 } },
        "layers": [{ "id": "sprite", "type": "image", "image": "sheet", "size": [40, 40] },
            solid("w", json!({ "effects": [{ "effect": "waves" }] }))] });
    let mut runtime =
        Runtime::new(Arc::new(Scene::from_json(&text.to_string()).unwrap()), Default::default());
    runtime.set_wall(90.0);
    let wake = runtime.frame(VIEW, Quality::Q3).wake;
    assert!(wake != Wake::Idle, "{wake:?}");
    runtime.set_reduced_motion(true);
    for _ in 0..3 {
        let wake = runtime.frame(VIEW, Quality::Q3).wake;
        assert_eq!(runtime.calmed(wake), Wake::At(30.0), "{wake:?}");
    }
    assert_eq!(runtime.calmed(Wake::At(3600.0)), Wake::At(3600.0));
    assert_eq!(runtime.calmed(Wake::Idle), Wake::Idle);
}
