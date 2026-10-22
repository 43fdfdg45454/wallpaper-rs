use serde_json::{Value, json};
use wprs_core::edit::{Depth, Edit, Mask, Motion, Parallax};
use wprs_core::motion::{Spring, Tilt, gravity, mount_matrix, orbit};

fn act(edit: &mut Edit, action: Value) -> Result<(), String> {
    edit.act(serde_json::from_value(action).unwrap()).map_err(|e| e.to_string())
}

#[test]
fn depth_and_parallax_sections_round_trip_and_are_checked() {
    let text = r#"{"version": 1,
      "masks": {"back": {"type": "depth", "range": [0.0, 0.35], "soft": 0.05}},
      "ops": [{"id": "bl", "type": "blur", "radius": 18, "mask": "back"}],
      "depth": {"model": "depth-anything-v2-small", "input": 518, "near": 0.98, "far": 0.02, "gamma": 1.0, "smooth": 2, "dilate": 3},
      "parallax": {"source": "pointer", "strength": 0.025, "focus": 0.3, "steps": 16, "smoothing_ms": 150, "overscan": 1.04}}"#;
    let edit = Edit::parse(text).unwrap();
    assert_eq!(edit.masks["back"], Mask::Depth { range: [0.0, 0.35], soft: 0.05, invert: false });
    assert_eq!(edit.parallax.as_ref().map(|p| (p.source, p.enabled)), Some((Motion::Pointer, true)));
    assert_eq!(Edit::parse(&edit.to_json()).unwrap(), edit);
    assert!(edit.needs_depth() && edit.moving().is_some() && !Edit::default().needs_depth());
    let broken = |path: &str, value: &str| {
        let mut e = edit.clone();
        e.set(path, value).unwrap_err()
    };
    assert_eq!(broken("parallax.strength", "0.2"), "parallax.strength: 0.2 is out of range 0 to 0.03");
    assert_eq!(broken("parallax.steps", "12"), "parallax.steps: use 1, 8, 16 or 32");
    assert_eq!(broken("depth.input", "500"), "depth.input: use a multiple of 14 from 196 to 1036");
    assert_eq!(broken("depth.model", "depth-anything-v2-large"), "depth.model: use depth-anything-v2-small");
    assert!(Edit::parse(r#"{"version": 1, "parallax": {"source": "gyro"}}"#).is_err());
}

#[test]
fn actions_adjust_depth_and_parallax() {
    let mut e = Edit::default();
    act(&mut e, json!({"depth": {"near": 0.9, "dilate": 5}})).unwrap();
    assert_eq!(e.depth, Some(Depth { near: 0.9, dilate: 5, ..Depth::default() }));
    act(&mut e, json!({"parallax": {"source": "accelerometer", "steps": 32}})).unwrap();
    let p = e.parallax.clone().unwrap();
    assert_eq!((p.source, p.steps, p.strength, p.enabled), (Motion::Accelerometer, 32, 0.02, true));
    act(&mut e, json!({"parallax": {"off": true}})).unwrap();
    assert!(e.moving().is_none() && !e.needs_depth());
    assert!(act(&mut e, json!({"parallax": {"source": "mouse"}})).unwrap_err().contains("not auto"));
    assert!(act(&mut e, json!({"parallax": {"steps": 3}})).is_err());
    act(&mut e, json!({"mask": {"name": "far", "spec": "depth:0,0.4"}})).unwrap();
    assert_eq!(e.masks["far"].spec().as_deref(), Some("depth:0,0.4,0.1"));
    e.set("parallax.focus", "0.6").unwrap();
    assert_eq!(e.parallax.as_ref().map(|p| p.focus), Some(0.6));
    let mut fresh = Edit::default();
    fresh.set("parallax.strength", "0.01").unwrap();
    assert_eq!(fresh.parallax, Some(Parallax { strength: 0.01, ..Parallax::default() }));
    act(&mut e, json!({"reset": {}})).unwrap();
    assert!(e.is_empty());
}

#[test]
fn the_spring_settles_on_its_goal_without_overshoot() {
    let mut spring = Spring::default();
    let mut steps = 0;
    while spring.step([1.0, -0.5], 0.15, 1.0 / 30.0) {
        assert!(spring.at[0] <= 1.0 + 1e-6 && spring.at[1] >= -0.5 - 1e-6, "{spring:?}");
        steps += 1;
    }
    assert_eq!(spring.at, [1.0, -0.5]);
    assert!((10..60).contains(&steps), "{steps}");
    assert!(!spring.step([1.0, -0.5], 0.15, 1.0 / 30.0) && spring.settled([1.0, -0.5]));
    let mut quick = Spring::default();
    quick.step([1.0, 0.0], 0.0, 1.0 / 30.0);
    assert_eq!(quick.at, [1.0, 0.0]);
}

#[test]
fn the_orbit_is_slow_and_bounded() {
    let samples: Vec<[f32; 2]> = (0..600).map(|i| orbit(i as f32 / 10.0)).collect();
    assert!(samples.iter().all(|p| p[0].abs() <= 1.0 && p[1].abs() <= 0.7));
    assert!(samples.windows(2).all(|w| (w[1][0] - w[0][0]).abs() < 0.05));
}

#[test]
fn iio_values_become_a_recentred_tilt() {
    let matrix = mount_matrix("0, 1, 0; -1, 0, 0; 0, 0, 1").unwrap();
    let g = gravity([100.0, 0.0, 980.0], 0.01, &matrix);
    assert!(g.iter().zip([0.0, -1.0, 9.8]).all(|(a, b)| (a - b).abs() < 1e-4), "{g:?}");
    assert!(mount_matrix("1, 0; 0, 1").is_none() && mount_matrix("a, 0, 0; 0, 1, 0; 0, 0, 1").is_none());
    let mut tilt = Tilt::default();
    assert_eq!(tilt.follow([0.0, 0.0, 9.8], 0.03), [0.0, 0.0]);
    let tipped = tilt.follow([2.5, 0.0, 9.5], 0.03);
    assert!(tipped[0] > 0.5 && tipped[1].abs() < 1e-3, "{tipped:?}");
    let later = (0..1000).fold(tipped, |_, _| tilt.follow([2.5, 0.0, 9.5], 0.03));
    assert!(later[0].abs() < 0.01, "{later:?}");
}
