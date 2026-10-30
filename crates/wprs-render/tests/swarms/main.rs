use image::{Rgb, RgbImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets, SceneView};
use wprs_scene::{Census, Quality, Runtime, SCHEMA, Scene, Values};

mod families;
mod looks;

const SIZE: (u32, u32) = (160, 80);

fn renderer() -> Option<&'static Renderer> {
    match Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

struct Run {
    runtime: Runtime,
    view: SceneView,
}

impl Run {
    fn new(r: &Renderer, systems: Value, layers: Value, gpu: bool) -> Option<Run> {
        Run::with(r, (systems, layers, json!({})), Vec::new(), gpu)
    }

    fn with(
        r: &Renderer,
        (systems, layers, assets): (Value, Value, Value),
        images: Vec<(&str, Arc<Pixels>)>,
        gpu: bool,
    ) -> Option<Run> {
        let canvas = json!({ "size": [SIZE.0, SIZE.1], "background": "#000000" });
        let text = json!({ "schema": SCHEMA, "canvas": canvas, "layers": layers, "particles": systems, "assets": assets });
        let scene = Scene::load(&text.to_string(), &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"));
        let images = images.into_iter().map(|(id, p)| (id.to_string(), p)).collect();
        let mut view =
            r.scene_view(&scene, &SceneAssets { images, ..Default::default() }, &Values::new()).unwrap();
        if !view.set_gpu_particles(r, gpu) {
            eprintln!("skipping: no compute shaders on {}", r.adapter());
            return None;
        }
        Some(Run { runtime: Runtime::new(Arc::new(scene), Values::new()), view })
    }

    fn frame(&mut self, r: &Renderer, dt: f64) -> Vec<u8> {
        self.runtime.advance(dt);
        let frame = self.runtime.frame(self.view.view(SIZE), Quality::Q3);
        let image = r.render_scene(&mut self.view, &self.runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
        let (mut target, mut pixels) = (r.create_target(SIZE.0, SIZE.1).unwrap(), Vec::new());
        let layer = Layer {
            image: Some(image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv: None,
            relief: None,
        };
        r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
        pixels
    }

    fn census(&self, r: &Renderer) -> Vec<Census> {
        r.census(&self.view).unwrap().into_iter().map(|(_, c)| c).collect()
    }
}

fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [pixels[i + 2], pixels[i + 1], pixels[i]]
}

fn looks_like(name: &str, pixels: &[u8]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/swarms/{name}.png"));
    let shot = RgbImage::from_fn(SIZE.0, SIZE.1, |x, y| Rgb(at(pixels, x, y)));
    if std::env::var_os("WPRS_BLESS").is_some() {
        return shot.save(&path).unwrap();
    }
    let reference = image::open(&path).unwrap().into_rgb8();
    let near = |a: &Rgb<u8>, b: &Rgb<u8>| a.0.iter().zip(b.0).all(|(x, y)| x.abs_diff(y) <= 24);
    let off = shot.pixels().zip(reference.pixels()).filter(|(a, b)| !near(a, b)).count();
    assert!(off <= 40, "{name}: {off} pixels differ from tests/swarms/{name}.png");
}
