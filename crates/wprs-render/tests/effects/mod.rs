use image::{Rgba, RgbaImage};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Renderer, SceneAssets, SceneStats, SceneView};
use wprs_scene::{Descriptor, Lut, PointerState, Quality, Runtime, SCHEMA, Scene, Values, register};

include!(concat!(env!("OUT_DIR"), "/effect_tests.rs"));

pub const W: u32 = 64;
pub const H: u32 = 32;
pub const RED: [u8; 3] = [255, 0, 0];
pub const BLUE: [u8; 3] = [0, 0, 255];

pub struct Fx {
    effects: Value,
    input: RgbaImage,
    assets: Map<String, Value>,
    files: SceneAssets,
    adjust: bool,
    quality: Quality,
}

pub struct Run {
    r: Renderer,
    pub runtime: Runtime,
    pub view: SceneView,
    quality: Quality,
}

pub struct Shot(pub RgbaImage);

pub fn probe(json: Value, wgsl: &str) {
    register(Descriptor::parse(&json.to_string(), wgsl).unwrap()).unwrap();
}

pub fn solid([r, g, b]: [u8; 3]) -> RgbaImage {
    RgbaImage::from_pixel(W, H, Rgba([r, g, b, 255]))
}

pub fn halves([r, g, b]: [u8; 3], [s, t, u]: [u8; 3]) -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, _| Rgba(if x < W / 2 { [r, g, b, 255] } else { [s, t, u, 255] }))
}

pub fn ramp() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, _| {
        let v = (x * 255 / (W - 1)) as u8;
        Rgba([v, v, v, 255])
    })
}

impl Fx {
    pub fn new(effect: &str, params: Value) -> Fx {
        Fx::chain(json!([{ "effect": effect, "params": params }]))
    }

    pub fn chain(effects: Value) -> Fx {
        let (assets, files) = Default::default();
        Fx { effects, input: ramp(), assets, files, adjust: false, quality: Quality::Q3 }
    }

    pub fn input(self, input: RgbaImage) -> Fx {
        Fx { input, ..self }
    }

    pub fn adjust(self) -> Fx {
        Fx { adjust: true, ..self }
    }

    pub fn quality(self, quality: Quality) -> Fx {
        Fx { quality, ..self }
    }

    pub fn image(mut self, id: &str, image: RgbaImage, linear: bool) -> Fx {
        self.assets.insert(id.into(), json!({ "path": format!("{id}.png"), "linear": linear }));
        self.files.images.insert(id.into(), Arc::new(image));
        self
    }

    pub fn lut(mut self, id: &str, text: &str) -> Fx {
        self.assets.insert(id.into(), json!({ "path": format!("{id}.cube") }));
        self.files.luts.insert(id.into(), Arc::new(Lut::parse(text.as_bytes()).unwrap()));
        self
    }

    pub fn run(self) -> Option<Run> {
        let r = super::renderer()?;
        let (effects, adjustment) = match self.adjust {
            true => (json!([]), Some(json!({ "id": "fx", "type": "adjustment", "effects": self.effects }))),
            false => (self.effects, None),
        };
        let image =
            json!({ "id": "src", "type": "image", "image": "src", "size": [W, H], "effects": effects });
        let layers: Vec<Value> = std::iter::once(image).chain(adjustment).collect();
        let mut assets = self.assets;
        assets.insert("src".into(), json!({ "path": "src.png" }));
        let canvas = json!({ "size": [W, H], "background": "#000000" });
        let text = json!({ "schema": SCHEMA, "canvas": canvas, "assets": assets, "layers": layers });
        let scene = Scene::load(&text.to_string(), &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"));
        let mut files = self.files;
        files.images.insert("src".into(), Arc::new(self.input));
        let view = r.scene_view(&scene, &files, &Values::new()).unwrap();
        Some(Run { r, runtime: Runtime::new(Arc::new(scene), Values::new()), view, quality: self.quality })
    }

    pub fn shot(self) -> Option<Shot> {
        Some(self.run()?.shot())
    }
}

impl Run {
    pub fn shot(&mut self) -> Shot {
        self.shot_at((W, H))
    }

    pub fn shot_at(&mut self, (w, h): (u32, u32)) -> Shot {
        let frame = self.runtime.frame(self.view.view((w, h)), self.quality);
        let image =
            self.r.render_scene(&mut self.view, &self.runtime, &frame, (w, h), self.quality, &[]).unwrap();
        let (mut target, mut pixels) = (self.r.create_target(w, h).unwrap(), Vec::new());
        let layer = Layer {
            image: Some(image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv: None,
            relief: None,
        };
        self.r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
        Shot(RgbaImage::from_fn(w, h, |x, y| {
            let i = ((y * w + x) * 4) as usize;
            Rgba([pixels[i + 2], pixels[i + 1], pixels[i], pixels[i + 3]])
        }))
    }

    pub fn stats(&self) -> SceneStats {
        self.view.stats()
    }

    pub fn pointer(&mut self, at: [f32; 2]) {
        let still = PointerState {
            pos: Some(at),
            smooth: at,
            inside: true,
            presence: 1.0,
            ..PointerState::default()
        };
        self.runtime.set_pointer(still);
    }
}

impl Shot {
    pub fn at(&self, x: u32, y: u32) -> [u8; 3] {
        let [r, g, b, _] = self.0.get_pixel(x, y).0;
        [r, g, b]
    }

    pub fn check(&self, points: &[((u32, u32), [u8; 3])]) {
        for &((x, y), expected) in points {
            let found = self.at(x, y);
            let near = found.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3);
            assert!(near, "pixel {x},{y} is {found:?}, expected {expected:?}");
        }
    }

    pub fn mean(&self) -> [f32; 3] {
        let n = (W * H) as f32;
        [0, 1, 2].map(|c| self.0.pixels().map(|p| f32::from(p.0[c])).sum::<f32>() / n)
    }

    pub fn diff(&self, other: &Shot) -> f32 {
        let pairs = self.0.as_raw().iter().zip(other.0.as_raw());
        pairs.map(|(a, b)| f32::from(a.abs_diff(*b))).sum::<f32>() / self.0.as_raw().len() as f32
    }
}
