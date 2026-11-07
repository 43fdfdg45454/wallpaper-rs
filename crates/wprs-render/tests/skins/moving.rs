use super::kit::{SIDE, mesh, pixels, renderer, sketch};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_render::{Pixels, Renderer, SceneAssets};
use wprs_scene::{Quality, Runtime, SCHEMA, Scene, Values};

const FRAMES: usize = 8;

#[derive(Clone, Copy, Debug)]
enum Mask {
    Free,
    Picture,
    Layer,
}

fn stripes() -> RgbaImage {
    let color = |x: u32, y: u32| {
        if ((x + y) / 5).is_multiple_of(2) { [230, 120, 30, 255] } else { [20, 40, 220, 255] }
    };
    RgbaImage::from_fn(SIDE, SIDE, |x, y| Rgba(color(x, y)))
}

fn arm() -> RgbaImage {
    RgbaImage::from_fn(SIDE, SIDE, |x, _| Rgba(if x < SIDE / 2 { [0, 0, 0, 255] } else { [255; 4] }))
}

fn swing(mut layer: Value) -> Value {
    layer["clips"] =
        json!([{ "id": "swing", "loop": "pingpong", "bones": { "arm": { "rotation": [[0, 0], [1, 60]] } } }]);
    layer["animations"] = json!([{ "clip": "swing" }]);
    layer
}

fn layers(amplitude: f64, grouped: bool, mask: Mask) -> Value {
    let params = json!({ "amplitude": amplitude, "frequency": 2, "speed": 1 });
    let mut effect = json!({ "effect": "waves", "params": params });
    match mask {
        Mask::Free => {}
        Mask::Picture => effect["mask"] = json!({ "texture": "ink", "channel": "r" }),
        Mask::Layer => effect["mask"] = json!({ "layer": "ink", "channel": "r" }),
    }
    let mut all = Vec::new();
    if matches!(mask, Mask::Layer) {
        let hidden = json!({ "id": "ink", "texture": "ink", "visible": false });
        all.push(swing(mesh(hidden)));
    }
    let mut fox = swing(mesh(json!({})));
    if grouped {
        all.push(json!({ "id": "all", "type": "group", "effects": [effect], "layers": [fox] }));
    } else {
        fox["effects"] = json!([effect]);
        all.push(fox);
    }
    Value::Array(all)
}

fn frames(r: &Renderer, amplitude: f64, grouped: bool, mask: Mask) -> (Vec<Vec<u8>>, Vec<u64>) {
    let assets =
        json!({ "pic": { "path": "pic.png" }, "ink": { "path": "ink.png" }, "mesh": { "path": "mesh.glb" } });
    let canvas = json!({ "size": [SIDE, SIDE], "background": "#000000" });
    let doc = json!({ "schema": SCHEMA, "canvas": canvas, "assets": assets, "layers": layers(amplitude, grouped, mask) });
    let mut doc = Scene::from_json(&doc.to_string()).unwrap();
    let glb = sketch(false);
    doc.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    let images =
        [("pic".to_string(), Pixels::new(stripes())), ("ink".to_string(), Pixels::new(arm()))].into();
    let mut view = r.scene_view(&doc, &SceneAssets { images, ..Default::default() }, &Values::new()).unwrap();
    let mut runtime = Runtime::new(Arc::new(doc), Values::new());
    let mut poses = Vec::new();
    let shots = (0..FRAMES).map(|_| {
        runtime.advance(1.0 / 30.0);
        let frame = runtime.frame(view.view((SIDE, SIDE)), Quality::Q3);
        let image = r.render_scene(&mut view, &runtime, &frame, (SIDE, SIDE), Quality::Q3, &[]).unwrap();
        poses.push(runtime.puppet("ink").map_or(0, |p| p.posed().generation));
        pixels(r, image)
    });
    (shots.collect::<Vec<_>>(), poses)
}

fn moved(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x.iter().zip(*y).any(|(p, q)| p.abs_diff(*q) > 24)).count()
}

#[test]
fn waves_on_a_mesh_whose_pose_changes_every_frame_bend_it_as_they_do_on_a_group_holding_it() {
    let Some(_) = renderer() else { return };
    let Ok(r) = Renderer::new() else { return };
    for tiled in [false, true] {
        r.force_tiled(tiled);
        for mask in [Mask::Free, Mask::Picture, Mask::Layer] {
            let (flat, _) = frames(&r, 0.0, false, mask);
            let (direct, _) = frames(&r, 8.0, false, mask);
            let (still, _) = frames(&r, 0.0, true, mask);
            let (grouped, _) = frames(&r, 8.0, true, mask);
            for k in 0..FRAMES {
                let (d, g) = (moved(&flat[k], &direct[k]), moved(&still[k], &grouped[k]));
                assert!(g > 40, "the group waves at frame {k} of {mask:?}: {g}");
                assert!(
                    d * 10 > g * 9 && g * 10 > d * 9,
                    "{d} pixels on the mesh, {g} on the group, {mask:?} {tiled} frame {k}"
                );
            }
        }
    }
}

#[test]
fn a_hidden_mesh_that_masks_an_effect_is_posed_every_frame_and_the_wave_follows_it() {
    let Some(_) = renderer() else { return };
    let Ok(r) = Renderer::new() else { return };
    for tiled in [false, true] {
        r.force_tiled(tiled);
        let (_, poses) = frames(&r, 8.0, false, Mask::Layer);
        assert!(poses.windows(2).all(|w| w[0] < w[1]), "the pose of the matte stood still: {poses:?}");
        let (layer, _) = frames(&r, 8.0, false, Mask::Layer);
        let (picture, _) = frames(&r, 8.0, false, Mask::Picture);
        let apart = moved(&layer[FRAMES - 1], &picture[FRAMES - 1]);
        assert!(
            apart > 20,
            "a matte that moves with the arm acts like one that does not, tiled {tiled}: {apart}"
        );
    }
}
