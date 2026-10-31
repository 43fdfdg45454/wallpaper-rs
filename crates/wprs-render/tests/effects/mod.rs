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
