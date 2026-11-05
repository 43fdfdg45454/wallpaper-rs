use super::kit::{at, close, layers};
use serde_json::json;
use wprs_scene::{Runtime, Value, Vec2};

fn gusty(gusts: f32) -> Runtime {
    let rope = json!([{ "id": "fox", "type": "mesh", "mesh": "arm", "size": [64, 64],
        "physics": [{ "bone": "root", "mode": "rope", "gravity": 0.5, "stiffness": 0, "damping": 0.2, "wind": 1 }] }]);
    let mut scene = (*layers(rope).scene).clone();
    scene.canvas.wind = Some(Value::Fixed(Vec2([300.0, 0.0])));
    scene.canvas.gusts = Some(gusts);
    Runtime::new(std::sync::Arc::new(scene), Default::default())
}

fn path(fps: f32) -> Vec<[f32; 2]> {
    let mut rt = gusty(1.0);
    let frames = (fps * 3.0).round() as usize;
    let every = (fps / 10.0).round() as usize;
    (1..=frames)
        .filter_map(|k| {
            rt.advance(f64::from(1.0 / fps));
            (k % every == 0).then(|| at(&rt, "hand"))
        })
        .collect()
}

#[test]
fn gusts_change_with_every_step_alike_at_every_frame_rate() {
    let reference = path(60.0);
    for fps in [30.0] {
        let other = path(fps);
        assert_eq!(other.len(), reference.len());
        for (k, (a, b)) in reference.iter().zip(&other).enumerate() {
            assert!(close(*a, *b, 2e-3), "{fps} fps differs at {k}: {a:?} {b:?}");
        }
    }
}

#[test]
fn full_gusts_drop_the_wind_between_them_and_none_keep_it_steady() {
    let mut steady = gusty(0.0);
    let mut gusts = gusty(1.0);
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for k in 0..600 {
        steady.advance(1.0 / 60.0);
        gusts.advance(1.0 / 60.0);
        if k > 300 {
            let x = at(&gusts, "hand")[0];
            (lo, hi) = (lo.min(x), hi.max(x));
        }
    }
    let x = at(&steady, "hand")[0];
    steady.advance(1.0 / 60.0);
    assert!((at(&steady, "hand")[0] - x).abs() < 0.05, "a steady wind settles");
    assert!(hi - lo > 1.0, "gusts keep the rope moving: {lo} {hi}");
}
