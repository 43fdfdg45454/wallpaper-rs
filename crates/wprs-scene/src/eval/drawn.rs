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

#[derive(Debug, Clone, PartialEq)]
pub struct Draw<'a> {
    pub layer: &'a Layer,
    pub end: usize,
    pub world: Affine,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame<'a> {
    pub draws: Vec<Draw<'a>>,
    pub effects: Vec<EffectDraw<'a>>,
    pub time: f32,
    pub view: [f32; 4],
    pub background: Color,
    pub quality: Quality,
    pub wake: Wake,
}
