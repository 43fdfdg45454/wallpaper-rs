use super::fields::{agree, alive, system};
use super::*;
use image::{Rgba, RgbaImage};

#[test]
fn particles_bounce_off_the_opaque_shape_of_a_layer_on_both_paths() {
    let Some(r) = renderer() else { return };
    let block = RgbaImage::from_fn(40, 80, |_, _| Rgba([255, 255, 255, 255]));
    let init = json!({ "life": [6, 6], "speed": [60, 60], "angle": [0, 0] });
    let wall = json!([{ "bounds": { "layer": { "id": "block" } }, "bounce": 1 }]);
    let systems =
        json!({ "s": system(json!({ "line": [[10, 10], [10, 70]] }), init, json!({ "collide": wall })) });
    let layers = json!([{ "id": "block", "type": "image", "image": "pic", "transform": { "position": [120, 40] } },
        { "id": "p", "type": "particles", "system": "s" }]);
    let assets = json!({ "pic": { "path": "pic.png" } });
    let mut found = [Vec::new(), Vec::new()];
    for (k, gpu) in [false, true].into_iter().enumerate() {
        let picture = vec![("pic", Pixels::new(block.clone()))];
        let Some(mut run) = Run::with(r, (systems.clone(), layers.clone(), assets.clone()), picture, gpu)
        else {
            return;
        };
        (0..60).for_each(|_| drop(run.frame(r, 1.0 / 30.0)));
        found[k] = alive(r, &run);
        let old: Vec<_> =
            found[k].iter().filter(|p| p.age > 1.6 && (20.0..60.0).contains(&p.pos[1])).collect();
        assert!(
            !old.is_empty() && old.iter().all(|p| p.pos[0] < 101.0 && p.vel[0] < 0.0),
            "gpu {gpu}: {old:?}"
        );
    }
    agree("layer wall", &found, 0.5);
}

#[test]
fn ropes_and_textured_long_trails_draw_along_the_path() {
    let Some(r) = renderer() else { return };
    let init = json!({ "life": [4, 4], "speed": [40, 40], "angle": [0, 0], "color": ["#ff0000", "#ff0000"] });
    for (name, render, gpu) in [
        ("rope", json!({ "rope": { "width": 6, "subdivide": 2 } }), false),
        ("rope", json!({ "rope": { "width": 6, "subdivide": 2 } }), true),
        (
            "trail",
            json!({ "trail": { "length": 48, "width": 1, "segments": 8, "subdivide": 3, "textured": true } }),
            false,
        ),
        (
            "trail",
            json!({ "trail": { "length": 48, "width": 1, "segments": 8, "subdivide": 3, "textured": true } }),
            true,
        ),
    ] {
        let mut s = system(json!({ "point": [20, 40] }), init.clone(), json!({ "render": render }));
        s["emitters"][0] = json!({ "shape": { "point": [20, 40] }, "rate": 30, "max_per_step": 1 });
        s["init"]["size"] = json!([0, 0]);
        if name == "trail" {
            s["emitters"][0] = json!({ "shape": { "point": [20, 40] }, "burst": { "count": 1 } });
            s["init"]["size"] = json!([8, 8]);
        }
        let layers = json!([{ "id": "p", "type": "particles", "system": "s", "blend": "add" }]);
        let Some(mut run) = Run::new(r, json!({ "s": s }), layers, gpu) else { return };
        run.view.set_particle_trails(r, wprs_scene::Tier::T3);
        let image = (0..60).map(|_| run.frame(r, 1.0 / 30.0)).last().unwrap();
        let lit = |x: u32| at(&image, x, 40)[0] > 60;
        let span = if name == "trail" { 72..96 } else { 30..90 };
        assert!(span.step_by(6).all(lit), "{name} gpu {gpu}: the path is drawn");
        assert!(
            at(&image, 60, 20)[0] < 10 && at(&image, 150, 40)[0] < 10,
            "{name} gpu {gpu}: nothing off the path"
        );
    }
}
