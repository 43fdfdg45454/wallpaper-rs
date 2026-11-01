use super::kit::{ORANGE, RED, SIDE, mesh, near, picture, render, renderer, rgb, scene, sketch};
use serde_json::json;
use std::sync::Arc;
use wprs_render::{Pixels, SceneAssets};
use wprs_scene::{Quality, Runtime, Values, Wake};

fn lifted(mut glb: Vec<u8>, z: f32) -> Vec<u8> {
    let text = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let doc: serde_json::Value = serde_json::from_slice(&glb[20..20 + text]).unwrap();
    let count = doc["accessors"][0]["count"].as_u64().unwrap() as usize;
    let bin = 20 + text + 8;
    (0..count).for_each(|i| glb[bin + i * 12 + 8..bin + i * 12 + 12].copy_from_slice(&z.to_le_bytes()));
    glb
}

#[test]
fn parts_seen_from_behind_hide_only_when_culled() {
    let Some(r) = renderer() else { return };
    let front = render(r, scene(mesh(json!({ "depth": { "cull": true } })), sketch(false)), 0.0, Quality::Q3);
    let plain = render(r, scene(mesh(json!({})), sketch(false)), 0.0, Quality::Q3);
    let worst = front.iter().zip(&plain).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(worst <= 2, "front faces stay: {worst}");
    let flip = json!({ "clips": [{ "id": "flip", "bones": { "arm": { "scale": [[0, [-1, 1]]] } } }], "animations": [{ "clip": "flip" }] });
    let mut culled = flip.clone();
    culled["depth"] = json!({ "cull": true });
    let shown = render(r, scene(mesh(flip), sketch(false)), 0.0, Quality::Q3);
    let hidden = render(r, scene(mesh(culled), sketch(false)), 0.0, Quality::Q3);
    assert!(near(rgb(&shown, 12, 32), RED), "{:?}", rgb(&shown, 12, 32));
    assert!(!near(rgb(&hidden, 12, 32), RED), "{:?}", rgb(&hidden, 12, 32));
}

#[test]
fn depth_pushes_vertices_with_perspective() {
    let Some(r) = renderer() else { return };
    let flat = render(r, scene(mesh(json!({})), lifted(sketch(false), 20.0)), 0.0, Quality::Q3);
    assert!(near(rgb(&flat, 50, 32), RED), "without depth the height is ignored");
    let near_eye = render(
        r,
        scene(mesh(json!({ "depth": { "scale": 1 } })), lifted(sketch(false), 20.0)),
        0.0,
        Quality::Q3,
    );
    assert!(near(rgb(&near_eye, 50, 32), ORANGE), "{:?}", rgb(&near_eye, 50, 32));
    assert!(near(rgb(&near_eye, 56, 32), RED));
    let none = render(
        r,
        scene(mesh(json!({ "depth": { "scale": 0 } })), lifted(sketch(false), 20.0)),
        0.0,
        Quality::Q3,
    );
    let worst = none.iter().zip(&flat).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(worst <= 2, "a scale of 0 keeps it flat: {worst}");
}

#[test]
fn swinging_and_reaching_bones_settle_and_sleep() {
    let Some(r) = renderer() else { return };
    let rig = json!({ "physics": [{ "bone": "arm", "mode": "rope", "gravity": 3, "stiffness": 0 }],
        "ik": [{ "chain": ["root", "arm"], "target": [4, 40], "weight": 0.5 }] });
    let scene = scene(mesh(rig), sketch(false));
    let assets =
        SceneAssets { images: [("pic".to_string(), Pixels::new(picture()))].into(), ..Default::default() };
    let mut view = r.scene_view(&scene, &assets, &Values::new()).unwrap();
    let mut runtime = Runtime::new(Arc::new(scene), Values::new());
    let size = (SIDE, SIDE);
    let mut step = |runtime: &mut Runtime, dt: f64| {
        runtime.advance(dt);
        let frame = runtime.frame(view.view(size), Quality::Q3);
        r.render_scene(&mut view, runtime, &frame, size, Quality::Q3, &[]).unwrap();
        (frame.wake, view.damage())
    };
    let (wake, damage) = step(&mut runtime, 1.0 / 60.0);
    assert_eq!(wake, Wake::Frame);
    assert!(damage.is_none_or(|d| d.width > 0));
    (0..1200).for_each(|_| _ = step(&mut runtime, 1.0 / 60.0));
    let (wake, damage) = step(&mut runtime, 1.0 / 60.0);
    assert_eq!((wake, damage), (Wake::Idle, Some(wprs_core::Region::default())));
}
