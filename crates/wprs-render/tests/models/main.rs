use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets, SceneView};
use wprs_scene::{Forge, ForgeMaterial, ForgeShape, Quality, Runtime, SCHEMA, Scene, Values};

mod cached;
mod depth;
mod finish;
mod paths;
mod posed;
mod shaders;
mod shadows;
mod tones;

pub const SIDE: u32 = 64;

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

pub fn png(rgba: [u8; 4], side: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    RgbaImage::from_pixel(side, side, Rgba(rgba)).write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

pub fn unlit(name: &str, color: [f32; 4]) -> ForgeMaterial {
    ForgeMaterial { unlit: true, ..ForgeMaterial::new(name, color) }
}

pub fn cube(m: ForgeMaterial) -> Vec<u8> {
    Forge::default().material(m).part("box", ForgeShape::Cube, [0.0; 3], 2.0, 0).glb()
}

pub fn scene(extra: Value, layers: Value, models: BTreeMap<&'static str, Vec<u8>>) -> Scene {
    let assets: serde_json::Map<String, Value> =
        models.keys().map(|k| (k.to_string(), json!({ "path": format!("{k}.glb") }))).collect();
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [SIDE, SIDE], "background": "#000000" },
        "assets": assets, "layers": layers });
    for (k, v) in extra.as_object().unwrap() {
        match (k.as_str(), v.as_object()) {
            ("assets", Some(more)) => more.iter().for_each(|(a, p)| doc["assets"][a] = p.clone()),
            _ => doc[k] = v.clone(),
        }
    }
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    let files = move |path: &str, _: usize| {
        let id = path.trim_end_matches(".glb");
        models.get(id).cloned().ok_or_else(|| format!("no {path}"))
    };
    scene.adopt(&BTreeMap::new(), files).unwrap();
    scene
}

pub struct Shot {
    pub view: SceneView,
    pub runtime: Runtime,
}

pub fn shoot(r: &Renderer, scene: Scene, images: &[(&str, Arc<Pixels>)]) -> Shot {
    let images = images.iter().map(|(k, p)| (k.to_string(), p.clone())).collect();
    let assets = SceneAssets { images, ..Default::default() };
    let view = r.scene_view(&scene, &assets, &Values::new()).unwrap();
    Shot { view, runtime: Runtime::new(Arc::new(scene), Values::new()) }
}

impl Shot {
    pub fn frame(&mut self, r: &Renderer, dt: f64, quality: Quality) -> Vec<u8> {
        self.runtime.advance(dt);
        let size = (SIDE, SIDE);
        let frame = self.runtime.frame(self.view.view(size), quality);
        let image = r.render_scene(&mut self.view, &self.runtime, &frame, size, quality, &[]).unwrap();
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
}

pub fn render(r: &Renderer, scene: Scene, quality: Quality) -> Vec<u8> {
    shoot(r, scene, &[]).frame(r, 0.0, quality)
}

pub fn rgb(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SIDE + x) * 4) as usize;
    [px[i + 2], px[i + 1], px[i]]
}

pub fn near(a: [u8; 3], b: [u8; 3], by: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= by)
}
