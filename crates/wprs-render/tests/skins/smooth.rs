use super::kit::renderer;
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets};
use wprs_scene::{Cut, Quality, Runtime, SCHEMA, Scene, Sketch, Values};

const OUT: u32 = 128;
const PICTURE: u32 = 1024;
const RADIUS: f32 = 0.4;

fn disk() -> RgbaImage {
    let c = PICTURE as f32 / 2.0;
    RgbaImage::from_fn(PICTURE, PICTURE, |x, y| {
        let inside = (x as f32 + 0.5 - c).hypot(y as f32 + 0.5 - c) < RADIUS * PICTURE as f32;
        if inside { Rgba([255; 4]) } else { Rgba([0; 4]) }
    })
}

fn shown(r: &Renderer, quality: Quality, pic: serde_json::Value) -> Vec<u8> {
    let glb = Sketch::cut(&vec![255; (OUT * OUT) as usize], [OUT, OUT], Cut::Grid, 8).glb();
    let layer = json!({ "id": "fox", "type": "mesh", "mesh": "mesh", "texture": "pic", "size": [OUT, OUT] });
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [OUT, OUT], "background": "#000000" },
        "assets": { "pic": pic, "mesh": { "path": "mesh.glb" } }, "layers": [layer] });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    let assets =
        SceneAssets { images: [("pic".to_string(), Pixels::new(disk()))].into(), ..Default::default() };
    let mut view = r.scene_view(&scene, &assets, &Values::new()).unwrap();
    let runtime = Runtime::new(Arc::new(scene), Values::new());
    let inner = view.inner(r, (OUT, OUT), quality);
    let frame = runtime.frame(view.view(inner), quality);
    let image = r.render_scene(&mut view, &runtime, &frame, inner, quality, &[]).unwrap();
    let layer = Layer {
        image: Some(image),
        fit: Fit::Stretch,
        background: [0; 3],
        opacity: 1.0,
        uv: None,
        relief: None,
        flip: [false; 2],
        grade: None,
    };
    let (mut target, mut out) = (r.create_target(OUT, OUT).unwrap(), Vec::new());
    r.render_layers(&mut target, &[layer], &mut out).unwrap();
    out
}

fn jaggedness(px: &[u8]) -> f32 {
    let (c, edge) = (OUT as f32 / 2.0, RADIUS * OUT as f32);
    let mut bins = [(u8::MAX, 0u8); 48];
    for (i, p) in px.as_chunks::<4>().0.iter().enumerate() {
        let (x, y) = ((i as u32 % OUT) as f32 + 0.5, (i as u32 / OUT) as f32 + 0.5);
        let at = ((x - c).hypot(y - c) - edge + 3.0) * 8.0;
        if let Some(bin) = (at >= 0.0).then_some(at as usize).and_then(|k| bins.get_mut(k)) {
            *bin = (bin.0.min(p[1]), bin.1.max(p[1]));
        }
    }
    let spreads: Vec<f32> = bins.iter().filter(|b| b.1 >= b.0).map(|b| f32::from(b.1 - b.0)).collect();
    spreads.iter().sum::<f32>() / spreads.len().max(1) as f32
}

#[test]
fn a_mesh_keeps_smooth_edges_at_the_lowest_quality() {
    let Some(r) = renderer() else { return };
    let [low, high] =
        [Quality::Q0, Quality::Q3].map(|q| jaggedness(&shown(r, q, json!({ "path": "pic.png" }))));
    eprintln!("edge spread at q0 {low:.1}, at q3 {high:.1}");
    assert!(low < 30.0, "the edge of the mesh shows steps at q0: {low:.1} (q3: {high:.1})");
    assert!(high < 30.0, "the edge of the mesh shows steps at q3: {high:.1}");
}

#[test]
fn a_mesh_picture_the_author_keeps_without_mipmaps_is_not_mipped() {
    let Some(r) = renderer() else { return };
    let pic = |mipmaps: Option<bool>| match mipmaps {
        Some(m) => json!({ "path": "pic.png", "mipmaps": m }),
        None => json!({ "path": "pic.png" }),
    };
    let [auto, off, on] = [None, Some(false), Some(true)].map(|m| jaggedness(&shown(r, Quality::Q0, pic(m))));
    eprintln!("edge spread at q0: automatic {auto:.1}, mipmaps off {off:.1}, mipmaps on {on:.1}");
    assert!(off > auto + 10.0, "a picture marked without mipmaps was mipped: {off:.1} against {auto:.1}");
    assert!(on < 30.0, "a picture marked with mipmaps shows steps: {on:.1}");
}
