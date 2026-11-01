use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets};
use wprs_scene::{Cut, Quality, Runtime, SCHEMA, Scene, Sketch, SketchBone, Values};

pub const SIDE: u32 = 64;
pub const ORANGE: [u8; 3] = [230, 120, 30];
pub const RED: [u8; 3] = [220, 20, 20];
pub const BLUE: [u8; 3] = [20, 40, 220];

pub fn renderer() -> Option<&'static Renderer> {
    match Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

pub fn picture() -> RgbaImage {
    RgbaImage::from_fn(SIDE, SIDE, |x, y| {
        let c = match (x, y) {
            (48.., _) => RED,
            (..16, ..16) => BLUE,
            _ => ORANGE,
        };
        Rgba([c[0], c[1], c[2], 255])
    })
}

pub fn sketch(shapes: bool) -> Vec<u8> {
    let mut s = Sketch::cut(&vec![255; (SIDE * SIDE) as usize], [SIDE, SIDE], Cut::Grid, 16);
    let bone = |name: &str, parent, at| SketchBone { name: name.into(), parent, at };
    s.bones = vec![
        bone("root", None, [4.0, 32.0]),
        bone("arm", Some(0), [32.0, 32.0]),
        bone("tip", Some(1), [60.0, 32.0]),
    ];
    s.bind();
    if shapes {
        s.shapes = vec![("up".into(), vec![[0.0, -16.0]; s.points.len()])];
    }
    s.glb()
}

pub fn scene(layer: Value, glb: Vec<u8>) -> Scene {
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [SIDE, SIDE], "background": "#000000" },
        "assets": { "pic": { "path": "pic.png" }, "mesh": { "path": "mesh.glb" } }, "layers": [layer] });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    scene
}

pub fn mesh(extra: Value) -> Value {
    let mut layer =
        json!({ "id": "fox", "type": "mesh", "mesh": "mesh", "texture": "pic", "size": [SIDE, SIDE] });
    extra.as_object().unwrap().iter().for_each(|(k, v)| layer[k] = v.clone());
    layer
}

pub fn render(r: &Renderer, scene: Scene, at: f64, quality: Quality) -> Vec<u8> {
    let assets =
        SceneAssets { images: [("pic".to_string(), Pixels::new(picture()))].into(), ..Default::default() };
    let mut view = r.scene_view(&scene, &assets, &Values::new()).unwrap();
    let mut runtime = Runtime::new(Arc::new(scene), Values::new());
    runtime.seek(at);
    let size = (SIDE, SIDE);
    let frame = runtime.frame(view.view(size), quality);
    let image = r.render_scene(&mut view, &runtime, &frame, size, quality, &[]).unwrap();
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
    let (mut target, mut out) = (r.create_target(SIDE, SIDE).unwrap(), Vec::new());
    r.render_layers(&mut target, &[layer], &mut out).unwrap();
    out
}

pub fn rgb(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SIDE + x) * 4) as usize;
    [px[i + 2], px[i + 1], px[i]]
}

pub fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 4)
}
