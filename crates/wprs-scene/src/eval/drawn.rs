use crate::{
    Affine, Align, Blend, Color, Descriptor, EffectUse, Gradient, Layer, Layout, MAX_PARAMS, Mask, Quality,
    Shape, Spectrum, Wake,
};
use std::borrow::Cow;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub enum Content<'a> {
    Group {
        clip: bool,
    },
    Image {
        asset: Cow<'a, str>,
        size: Option<[f32; 2]>,
        frame: Option<u32>,
    },
    Video {
        asset: Cow<'a, str>,
        size: Option<[f32; 2]>,
    },
    Text {
        text: Cow<'a, str>,
        font: Option<&'a str>,
        fallback: &'a [String],
        size: f32,
        color: Color,
        align: Align,
        layout: Layout,
        ink: Option<crate::Ink>,
    },
    Shape {
        shape: Shape,
        fill: Option<Color>,
        gradient: Option<&'a Gradient>,
        stroke: Option<(Color, f32)>,
    },
    Spectrum {
        spectrum: &'a Spectrum,
        color: Color,
    },
    Solid {
        color: Color,
        size: [f32; 2],
    },
    Particles {
        system: &'a str,
        flock: usize,
        size: f32,
    },
    Mesh {
        layer: &'a str,
        size: [f32; 2],
        pose: u64,
        texture: Option<Cow<'a, str>>,
        relief: Option<crate::Relief>,
    },
    Adjustment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectDraw<'a> {
    pub descriptor: &'a Descriptor,
    pub effect: &'a EffectUse,
    pub slots: [[f32; 4]; MAX_PARAMS],
    pub mask: Option<&'a Mask>,
    pub mask_texture: Option<Cow<'a, str>>,
    pub mask_strength: f32,
    pub scale: f32,
    pub reach: f32,
    pub chain: usize,
    pub textures: Vec<(&'a str, String)>,
    pub velocity: [f32; 2],
    pub variant: u32,
    pub frozen: bool,
    pub reset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lit<'a> {
    pub strength: f32,
    pub roughness: f32,
    pub reflection: f32,
    pub emissive: f32,
    pub normal: Option<&'a str>,
    pub env: Option<&'a str>,
    pub roughness_map: Option<&'a str>,
    pub emissive_map: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Draw<'a> {
    pub layer: &'a Layer,
    pub end: usize,
    pub world: Affine,
    pub lean: Option<crate::Lean>,
    pub origin: [f32; 2],
    pub opacity: f32,
    pub blend: Blend,
    pub mask: Option<&'a Mask>,
    pub mask_texture: Option<Cow<'a, str>>,
    pub mask_strength: f32,
    pub tint: Color,
    pub brightness: f32,
    pub effects: Range<usize>,
    pub content: Content<'a>,
    pub dynamic: bool,
    pub isolate: bool,
    pub hidden: bool,
    pub lit: Option<Lit<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame<'a> {
    pub draws: Vec<Draw<'a>>,
    pub effects: Vec<EffectDraw<'a>>,
    pub time: f32,
    pub view: [f32; 4],
    pub background: Color,
    pub camera: Affine,
    pub canvas: [f32; 2],
    pub keep: bool,
    pub quality: Quality,
    pub wake: Wake,
    pub lamps: Vec<super::lamps::Lamp<'a>>,
    pub ambient: Option<Color>,
    pub sky: Option<Color>,
}
