use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets, SceneView};
use wprs_scene::{Quality, Runtime, SCHEMA, Scene, Values};

mod beams;
mod cached;
mod surfaces;

pub const SIZE: (u32, u32) = (160, 90);

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

pub fn flat(rgba: [u8; 4]) -> std::sync::Arc<Pixels> {
    Pixels::new(RgbaImage::from_pixel(32, 32, Rgba(rgba)))
}

pub fn scene(layers: Value, canvas: Value) -> Scene {
    let mut c = json!({ "size": [160, 90], "background": "#000000" });
    c.as_object_mut().unwrap().extend(canvas.as_object().cloned().unwrap_or_default());
    let assets = json!({ "wall": { "path": "wall.png" }, "bump": { "path": "bump.png", "linear": true },
        "env": { "path": "env.png" }, "mask": { "path": "mask.png", "linear": true } });
    let timelines = [json!({ "id": "go", "duration": 4, "loop": "repeat" })];
    let text =
        json!({ "schema": SCHEMA, "canvas": c, "assets": assets, "timelines": timelines, "layers": layers });
    Scene::load(&text.to_string(), &Default::default()).unwrap()
}

pub fn files(bump: [u8; 4]) -> SceneAssets {
    let images = [
        ("wall", flat([200, 200, 200, 255])),
        ("bump", flat(bump)),
        ("env", flat([0, 0, 255, 255])),
        ("mask", Pixels::new(RgbaImage::from_fn(32, 32, |x, _| Rgba([if x < 16 { 0 } else { 255 }; 4])))),
    ];
    SceneAssets {
        images: images.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        ..Default::default()
    }
}

pub fn wall(lighting: Value) -> Value {
    json!({ "id": "wall", "type": "image", "image": "wall", "size": [160, 90], "transform": { "position": [80, 45] },
        "lighting": lighting })
}

pub struct Shot {
    pub runtime: Runtime,
    pub view: SceneView,
}

impl Shot {
    pub fn new(r: &Renderer, scene: Scene, files: &SceneAssets) -> Shot {
        let view = r.scene_view(&scene, files, &Values::new()).unwrap();
        Shot { runtime: Runtime::new(Arc::new(scene), Values::new()), view }
    }

    pub fn pixels(&mut self, r: &Renderer, quality: Quality) -> RgbaImage {
        let frame = self.runtime.frame(self.view.view(SIZE), quality);
        let image = r.render_scene(&mut self.view, &self.runtime, &frame, SIZE, quality, &[]).unwrap();
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
        let (mut target, mut out) = (r.create_target(SIZE.0, SIZE.1).unwrap(), Vec::new());
        r.render_layers(&mut target, &[layer], &mut out).unwrap();
        RgbaImage::from_raw(SIZE.0, SIZE.1, out).unwrap()
    }
}

pub fn once(layers: Value, canvas: Value, bump: [u8; 4]) -> Option<RgbaImage> {
    let r = renderer()?;
    Some(Shot::new(r, scene(layers, canvas), &files(bump)).pixels(r, Quality::Q3))
}

pub fn at(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    image.get_pixel(x, y).0
}

pub const UP: [u8; 4] = [128, 128, 255, 255];
