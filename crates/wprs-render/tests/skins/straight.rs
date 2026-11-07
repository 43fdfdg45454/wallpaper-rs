use super::kit::{SIDE, mesh, picture, pixels, renderer, sketch};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_render::{Pixels, Renderer, SceneAssets};
use wprs_scene::{Quality, Runtime, SCHEMA, Scene, Values};

fn waves() -> Value {
    let params = json!({ "amplitude": 3, "frequency": 2, "speed": 0 });
    json!([{ "effect": "waves", "params": params, "mask": { "layer": "mask", "channel": "r" } }])
}

fn drawn(r: &Renderer, shown: Value, tiled: bool) -> (Vec<u8>, u32) {
    let hidden = mesh(json!({ "id": "mask", "visible": false }));
    let assets = json!({ "pic": { "path": "pic.png" }, "mesh": { "path": "mesh.glb" } });
    let canvas = json!({ "size": [SIDE, SIDE], "background": "#000000" });
    let doc = json!({ "schema": SCHEMA, "canvas": canvas, "assets": assets, "layers": [hidden, shown] });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    let glb = sketch(false);
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    r.force_tiled(tiled);
    let images = [("pic".to_string(), Pixels::new(picture()))].into();
    let mut view =
        r.scene_view(&scene, &SceneAssets { images, ..Default::default() }, &Values::new()).unwrap();
    let runtime = Runtime::new(Arc::new(scene), Values::new());
    let frame = runtime.frame(view.view((SIDE, SIDE)), Quality::Q3);
    let image = r.render_scene(&mut view, &runtime, &frame, (SIDE, SIDE), Quality::Q3, &[]).unwrap();
    (pixels(r, image), view.stats().passes)
}

fn apart(a: &[u8], b: &[u8]) -> (u8, f32) {
    let far = a.iter().zip(b).filter(|(x, y)| x.abs_diff(**y) > 24).count();
    (a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0), far as f32 / a.len() as f32)
}

#[test]
fn a_tiled_gpu_reads_pictures_and_hidden_mesh_masks_where_they_lie_instead_of_drawing_them_apart() {
    let Some(_) = renderer() else { return };
    let Ok(r) = Renderer::new() else { return };
    let picture = json!({ "id": "photo", "type": "image", "image": "pic", "size": [SIDE, SIDE] });
    for shown in [mesh(json!({ "effects": waves() })), picture] {
        let mut shown = shown;
        shown["effects"] = waves();
        let (plain, before) = drawn(&r, shown.clone(), false);
        let (tiled, after) = drawn(&r, shown.clone(), true);
        assert!(after + 2 <= before, "{after} passes on a tiled GPU, {before} elsewhere for {shown}");
        let (most, share) = apart(&plain, &tiled);
        assert!(share < 0.03, "{share} of the channels moved, at most by {most}, for {shown}");
    }
}
