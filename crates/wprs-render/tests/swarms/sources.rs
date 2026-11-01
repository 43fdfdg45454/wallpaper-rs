use super::fields::{agree, alive, system};
use super::*;
use image::{Rgba, RgbaImage};

fn scene(shape: Value, at: [f32; 2]) -> (Value, Value, Value) {
    let systems = json!({ "s": system(shape, json!({}), json!({})) });
    let layers = json!([{ "id": "src", "type": "image", "image": "pic", "transform": { "position": at } },
        { "id": "p", "type": "particles", "system": "s", "blend": "add" }]);
    (systems, layers, json!({ "pic": { "path": "pic.png" } }))
}

fn picture() -> RgbaImage {
    RgbaImage::from_fn(32, 16, |x, _| Rgba(if x < 16 { [255, 0, 0, 255] } else { [0, 0, 255, 40] }))
}

fn run(
    r: &Renderer,
    scene: (Value, Value, Value),
    gpu: bool,
    frames: usize,
) -> Option<(Run, Vec<wprs_scene::Particle>)> {
    let mut run = Run::with(r, scene, vec![("pic", Pixels::new(picture()))], gpu)?;
    (0..frames).for_each(|_| drop(run.frame(r, 1.0 / 30.0)));
    let found = alive(r, &run);
    Some((run, found))
}

#[test]
fn particles_are_born_on_the_opaque_pixels_of_a_layer_with_its_color() {
    let Some(r) = renderer() else { return };
    let shape = json!({ "layer": { "id": "src", "threshold": 0.5, "color": true } });
    let mut found = [Vec::new(), Vec::new()];
    for (k, gpu) in [false, true].into_iter().enumerate() {
        let Some((_, all)) = run(r, scene(shape.clone(), [80.0, 40.0]), gpu, 8) else { return };
        assert!(all.len() > 50, "gpu {gpu}: {}", all.len());
        for p in &all {
            assert!((63.0..81.0).contains(&p.pos[0]) && (31.0..49.0).contains(&p.pos[1]), "gpu {gpu}: {p:?}");
        }
        let red = all.iter().filter(|p| p.color[0] > 0.9 && p.color[2] < 0.1).count();
        assert!(red * 10 >= all.len() * 9, "gpu {gpu}: the color of the layer, {red} of {}", all.len());
        found[k] = all;
    }
    agree("layer", &found, 0.05);
}

#[test]
fn a_moving_layer_carries_its_births_and_their_motion() {
    let Some(r) = renderer() else { return };
    let shape = json!({ "layer": { "id": "src", "inherit": true, "jitter": 2 } });
    let (mut systems, mut layers, assets) = scene(shape, [40.0, 40.0]);
    layers[0]["transform"]["position"] = json!({ "keys": [[0, [40, 40]], [10, [140, 40]]] });
    systems["s"]["emitters"][0]["rate"] = json!(60);
    for gpu in [false, true] {
        let Some((_, all)) = run(r, (systems.clone(), layers.clone(), assets.clone()), gpu, 20) else {
            return;
        };
        let fresh: Vec<_> = all.iter().filter(|p| p.age < 0.1).collect();
        assert!(!fresh.is_empty(), "gpu {gpu}");
        let c = 40.0 + 10.0 * 20.0 / 30.0;
        let on = |p: &&&wprs_scene::Particle| {
            (c - 18.0..c + 2.0).contains(&p.pos[0]) && (p.vel[0] - 10.0).abs() < 1.0
        };
        assert!(fresh.iter().all(|p| on(&p)), "gpu {gpu}: {fresh:?}");
    }
}
