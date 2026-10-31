#[path = "../effects/mod.rs"]
mod effects;
#[path = "../looks/mod.rs"]
mod looks;

use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets, SceneView};
use wprs_scene::{AudioFrame, Property, Quality, Runtime, SCHEMA, Scene, effective};

mod compose;
mod passes;

const SIZE: (u32, u32) = (100, 50);

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

fn halves(left: [u8; 4], right: [u8; 4]) -> Arc<Pixels> {
    Pixels::new(RgbaImage::from_fn(100, 50, |x, _| Rgba(if x < 50 { left } else { right })))
}

struct Shot {
    runtime: Runtime,
    view: SceneView,
}

impl Shot {
    fn new(r: &Renderer, canvas: Value, layers: Value, images: &[(&str, Arc<Pixels>)]) -> Shot {
        let assets = images.iter().map(|(id, _)| (id.to_string(), json!({ "path": format!("{id}.png") })));
        Shot::with(
            r,
            json!({ "canvas": canvas, "layers": layers, "assets": assets.collect::<Value>() }),
            images,
        )
    }

    fn with(r: &Renderer, mut text: Value, images: &[(&str, Arc<Pixels>)]) -> Shot {
        text["schema"] = json!(SCHEMA);
        let props: BTreeMap<String, Property> =
            serde_json::from_value(json!({ "tone": { "type": "color", "default": "#00ff00" } })).unwrap();
        let scene = Scene::load(&text.to_string(), &props).unwrap();
        let assets = SceneAssets {
            images: images.iter().map(|(id, i)| (id.to_string(), i.clone())).collect(),
            ..Default::default()
        };
        let values = effective(&props, &[]);
        let view = r.scene_view(&scene, &assets, &values).unwrap();
        Shot { runtime: Runtime::new(Arc::new(scene), values), view }
    }

    fn pixels(&mut self, r: &Renderer) -> Vec<u8> {
        let frame = self.runtime.frame(self.view.view(SIZE), Quality::Q3);
        let image = r.render_scene(&mut self.view, &self.runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
        let mut target = r.create_target(SIZE.0, SIZE.1).unwrap();
        let mut pixels = Vec::new();
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
}

fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [pixels[i + 2], pixels[i + 1], pixels[i]]
}

fn near(found: [u8; 3], expected: [u8; 3], tolerance: u8) -> bool {
    found.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= tolerance)
}

fn check(pixels: &[u8], points: &[((u32, u32), [u8; 3])]) {
    for &((x, y), expected) in points {
        assert!(
            near(at(pixels, x, y), expected, 3),
            "pixel {x},{y} is {:?}, expected {expected:?}",
            at(pixels, x, y)
        );
    }
}

fn canvas(background: &str) -> Value {
    json!({ "size": [100, 50], "background": background })
}

fn fontdb_has_fonts() -> bool {
    ["/usr/share/fonts", "/usr/local/share/fonts"].iter().any(|d| std::path::Path::new(d).is_dir())
}
