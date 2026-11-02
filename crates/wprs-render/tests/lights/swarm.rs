use super::{Shot, at, files, renderer};
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_scene::{Quality, Runtime, SCHEMA, Scene, Values};

fn system(render: Value) -> Value {
    json!({ "capacity": 4, "seed": 2, "emitters": [{ "shape": { "point": [80, 45] }, "burst": { "count": 1 } }],
        "init": { "life": [30, 30], "speed": [0, 0], "size": [40, 40], "color": ["#ffffff", "#ffffff"] },
        "render": render })
}

fn lamp(x: f32) -> Value {
    json!({ "id": "lamp", "type": "light", "transform": { "position": [x, 45] }, "z": 20, "radius": 60, "intensity": 2 })
}

fn shot(render: Value, mut layers: Vec<Value>, gpu: bool, quality: Quality) -> Option<image::RgbaImage> {
    let r = renderer()?;
    layers.push(json!({ "id": "p", "type": "particles", "system": "s" }));
    let canvas = json!({ "size": [160, 90], "background": "#000000" });
    let assets = json!({ "wall": { "path": "wall.png" }, "bump": { "path": "bump.png", "linear": true },
        "mask": { "path": "mask.png", "linear": true } });
    let text = json!({ "schema": SCHEMA, "canvas": canvas, "assets": assets, "layers": layers,
        "particles": { "s": system(render) } });
    let scene = Scene::load(&text.to_string(), &Default::default()).unwrap();
    let view = r.scene_view(&scene, &files([255, 128, 128, 255]), &Values::new()).unwrap();
    let mut shot = Shot { runtime: Runtime::new(Arc::new(scene), Values::new()), view };
    if !shot.view.set_gpu_particles(r, gpu) {
        return None;
    }
    shot.pixels(r, quality);
    shot.runtime.advance(1.0 / 30.0);
    Some(shot.pixels(r, quality))
}

fn close(a: &image::RgbaImage, b: &image::RgbaImage) -> bool {
    a.pixels().zip(b.pixels()).all(|(p, q)| (0..4).all(|k| p.0[k].abs_diff(q.0[k]) <= 8))
}

#[test]
fn lit_particles_take_the_scene_lights_on_both_paths() {
    for gpu in [false, true] {
        let Some(plain) = shot(json!({}), vec![lamp(80.0)], gpu, Quality::Q3) else { continue };
        let Some(near) = shot(json!({ "lit": true }), vec![lamp(80.0)], gpu, Quality::Q3) else { continue };
        let Some(dark) = shot(json!({ "lit": true }), vec![lamp(400.0)], gpu, Quality::Q3) else { continue };
        assert_eq!(at(&plain, 80, 45)[0], 255, "gpu {gpu}");
        assert!(at(&near, 80, 45)[0] > 100, "gpu {gpu}: {:?}", at(&near, 80, 45));
        assert!(at(&dark, 80, 45)[0] < 10, "gpu {gpu}: {:?}", at(&dark, 80, 45));
    }
    let (Some(cpu), Some(gpu)) = (
        shot(json!({ "lit": true, "normal": "bump" }), vec![lamp(60.0)], false, Quality::Q3),
        shot(json!({ "lit": true, "normal": "bump" }), vec![lamp(60.0)], true, Quality::Q3),
    ) else {
        return;
    };
    assert!(close(&cpu, &gpu), "the CPU and GPU paths differ");
}

#[test]
fn a_normal_map_turns_particles_toward_the_light_on_both_sides() {
    let Some(right) = shot(json!({ "lit": true, "normal": "bump" }), vec![lamp(110.0)], false, Quality::Q3)
    else {
        return;
    };
    let Some(left) = shot(json!({ "lit": true, "normal": "bump" }), vec![lamp(50.0)], false, Quality::Q3)
    else {
        return;
    };
    assert!(
        at(&right, 80, 45)[0] > at(&left, 80, 45)[0] + 20,
        "{:?} {:?}",
        at(&right, 80, 45),
        at(&left, 80, 45)
    );
    let mut under = lamp(80.0);
    under["z"] = json!(-20);
    let Some(back) = shot(json!({ "lit": true }), vec![under], false, Quality::Q3) else { return };
    assert!(at(&back, 80, 45)[0] > 100, "a light behind lights the back face: {:?}", at(&back, 80, 45));
}

#[test]
fn refracting_particles_bend_what_is_behind_them_above_the_low_levels() {
    let back = json!({ "id": "back", "type": "image", "image": "mask", "size": [160, 90],
        "transform": { "position": [80, 45] } });
    let bend = json!({ "refract": { "normal": "bump", "amount": 1 } });
    for gpu in [false, true] {
        let Some(bent) = shot(bend.clone(), vec![back.clone()], gpu, Quality::Q3) else { continue };
        let Some(low) = shot(bend.clone(), vec![back.clone()], gpu, Quality::Q1) else { continue };
        assert!(at(&bent, 75, 45)[0] > 200, "gpu {gpu}: {:?}", at(&bent, 75, 45));
        assert!(at(&low, 75, 45)[0] < 10, "gpu {gpu}: {:?}", at(&low, 75, 45));
        let Some(half) = shot(bend.clone(), vec![back.clone()], gpu, Quality::Q2) else { continue };
        assert!(at(&half, 75, 45)[0] > 200, "gpu {gpu}: {:?}", at(&half, 75, 45));
    }
    let (Some(cpu), Some(gpu)) = (
        shot(bend.clone(), vec![back.clone()], false, Quality::Q3),
        shot(bend, vec![back], true, Quality::Q3),
    ) else {
        return;
    };
    assert!(close(&cpu, &gpu), "the CPU and GPU paths bend differently");
}
