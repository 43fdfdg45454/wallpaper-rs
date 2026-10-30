use image::{Rgba, RgbaImage};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets, SceneStats, SceneView};
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
    own: Vec<(String, Value, String)>,
    unchecked: Vec<(String, String)>,
    background: &'static str,
    more: Vec<Value>,
    space: &'static str,
}

pub struct Run {
    r: &'static Renderer,
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
        let (assets, files, own) = Default::default();
        Fx {
            effects,
            input: ramp(),
            assets,
            files,
            adjust: false,
            quality: Quality::Q3,
            own,
            unchecked: Vec::new(),
            background: "#000000",
            more: Vec::new(),
            space: "linear",
        }
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
        self.files.images.insert(id.into(), Pixels::new(image));
        self
    }

    pub fn background(self, background: &'static str) -> Fx {
        Fx { background, ..self }
    }

    pub fn layer(mut self, layer: Value) -> Fx {
        self.more.push(layer);
        self
    }

    pub fn space(self, space: &'static str) -> Fx {
        Fx { space, ..self }
    }

    pub fn own(mut self, json: Value, wgsl: &str) -> Fx {
        self.own.push((json["id"].as_str().unwrap_or_default().into(), json, wgsl.into()));
        self
    }

    pub fn unchecked(mut self, id: &str, wgsl: &str) -> Fx {
        self.unchecked.push((id.into(), wgsl.into()));
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
        let layers: Vec<Value> = std::iter::once(image).chain(self.more).chain(adjustment).collect();
        let mut assets = self.assets;
        assets.insert("src".into(), json!({ "path": "src.png" }));
        let canvas = json!({ "size": [W, H], "background": self.background, "blend_space": self.space });
        let own: Map<String, Value> = self
            .own
            .iter()
            .map(|(id, ..)| {
                (
                    id.clone(),
                    json!({ "descriptor": format!("fx/{id}.json"), "wgsl": format!("fx/{id}.wgsl") }),
                )
            })
            .collect();
        let text =
            json!({ "schema": SCHEMA, "canvas": canvas, "assets": assets, "layers": layers, "effects": own });
        let mut scene = Scene::from_json(&text.to_string()).unwrap_or_else(|e| panic!("{e}"));
        let files = |path: &str, _: usize| {
            let (id, ext) = path.trim_start_matches("fx/").split_once('.').unwrap_or_default();
            let (_, json, wgsl) = self.own.iter().find(|(i, ..)| i == id).ok_or("missing")?;
            Ok::<_, &str>(if ext == "json" { json.to_string() } else { wgsl.clone() }.into_bytes())
        };
        scene.adopt(&BTreeMap::new(), files).unwrap_or_else(|e| panic!("{e}"));
        for (id, wgsl) in &self.unchecked {
            let d = scene.loaded.get_mut(id).expect("a package effect");
            let key = d.key ^ wgsl.bytes().fold(0u64, |h, b| h.rotate_left(5) ^ u64::from(b)) << 1;
            *d = Arc::new(Descriptor { wgsl: wgsl.clone(), key, ..(**d).clone() });
        }
        let mut files = self.files;
        files.images.insert("src".into(), Pixels::new(self.input));
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

    pub fn shot_at(&mut self, size: (u32, u32)) -> Shot {
        self.shot_edited(size, |_| {})
    }

    pub fn shot_edited(&mut self, (w, h): (u32, u32), edit: impl FnOnce(&mut wprs_scene::Frame)) -> Shot {
        let mut frame = self.runtime.frame(self.view.view((w, h)), self.quality);
        edit(&mut frame);
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
    pub fn sheet(shots: &[Shot]) -> Shot {
        let mut sheet = RgbaImage::new(W, H * shots.len() as u32);
        for (n, shot) in shots.iter().enumerate() {
            image::imageops::replace(&mut sheet, &shot.0, 0, i64::from(n as u32 * H));
        }
        Shot(sheet)
    }

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

    pub fn looks_like(&self, name: &str) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/effects/{name}.png"));
        if std::env::var_os("WPRS_BLESS").is_some() {
            return self.0.save(&path).unwrap();
        }
        let reference = image::open(&path).unwrap().into_rgba8();
        let near = |a: &Rgba<u8>, b: &Rgba<u8>| a.0.iter().zip(b.0).all(|(x, y)| x.abs_diff(y) <= 24);
        let off = self.0.pixels().zip(reference.pixels()).filter(|(a, b)| !near(a, b)).count();
        let allowed = 8 * (self.0.height() / H).max(1) as usize;
        assert!(self.0.dimensions() == reference.dimensions(), "{name}: the size changed");
        assert!(off <= allowed, "{name}: {off} pixels differ from tests/effects/{name}.png");
    }

    pub fn diff(&self, other: &Shot) -> f32 {
        let pairs = self.0.as_raw().iter().zip(other.0.as_raw());
        pairs.map(|(a, b)| f32::from(a.abs_diff(*b))).sum::<f32>() / self.0.as_raw().len() as f32
    }
}
