use crate::input::LayerOverride;
use crate::resolve::Context;
use crate::{
    Align, Blend, Color, Descriptor, Gradient, Kind, Layer, Mask, Quality, Shape, Spectrum, descriptor,
};
use std::borrow::Cow;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wake {
    Frame,
    At(f64),
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine(pub [f32; 6]);

#[derive(Debug, Clone, PartialEq)]
pub enum Content<'a> {
    Group { clip: bool },
    Image { asset: Cow<'a, str>, size: Option<[f32; 2]> },
    Video { asset: &'a str, size: Option<[f32; 2]> },
    Text { text: Cow<'a, str>, font: Option<&'a str>, size: f32, color: Color, align: Align },
    Shape { shape: Shape, fill: Option<Color>, gradient: Option<&'a Gradient>, stroke: Option<(Color, f32)> },
    Spectrum { spectrum: &'a Spectrum, color: Color },
    Solid { color: Color, size: [f32; 2] },
    Particles { system: &'a str },
    Adjustment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectDraw<'a> {
    pub descriptor: &'static Descriptor,
    pub slots: [[f32; 4]; 16],
    pub mask: Option<&'a Mask>,
    pub scale: f32,
    pub reach: f32,
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
    pub wake: Wake,
}

impl Wake {
    pub fn min(self, other: Wake) -> Wake {
        match (self, other) {
            (Wake::Frame, _) | (_, Wake::Frame) => Wake::Frame,
            (Wake::At(a), Wake::At(b)) => Wake::At(a.min(b)),
            (Wake::At(a), Wake::Idle) | (Wake::Idle, Wake::At(a)) => Wake::At(a),
            (Wake::Idle, Wake::Idle) => Wake::Idle,
        }
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn then(self, inner: Affine) -> Affine {
        let ([a, b, c, d, e, f], [g, h, i, j, k, l]) = (self.0, inner.0);
        Affine([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ])
    }

    pub fn translate([x, y]: [f32; 2]) -> Affine {
        Affine([1.0, 0.0, 0.0, 1.0, x, y])
    }

    pub fn scale([x, y]: [f32; 2]) -> Affine {
        Affine([x, 0.0, 0.0, y, 0.0, 0.0])
    }

    pub fn rotate(degrees: f32) -> Affine {
        let (s, c) = degrees.to_radians().sin_cos();
        Affine([c, s, -s, c, 0.0, 0.0])
    }

    pub fn skew([x, y]: [f32; 2]) -> Affine {
        Affine([1.0, y.to_radians().tan(), x.to_radians().tan(), 1.0, 0.0, 0.0])
    }

    pub fn apply(self, [x, y]: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d, e, f] = self.0;
        [a * x + c * y + e, b * x + d * y + f]
    }
}

pub(crate) fn walk<'a>(
    cx: &Context,
    layers: &'a [Layer],
    parent: Affine,
    quality: Quality,
    frame: &mut Frame<'a>,
    canvas: [f32; 2],
) {
    let none = LayerOverride::default();
    for layer in layers {
        let set = cx.script.layers.get(&layer.id).unwrap_or(&none);
        let outer = cx.wake.replace(Wake::Idle);
        let visible = set.visible.unwrap_or_else(|| layer.visible.get(cx));
        let opacity = set.opacity.unwrap_or_else(|| layer.opacity.get(cx)).clamp(0.0, 1.0);
        let (shown, at) = (visible && opacity > 0.0, frame.draws.len());
        if shown || cx.mattes.contains(&layer.id) {
            emit(cx, layer, set, parent, opacity, quality, frame, canvas);
            frame.draws[at].hidden = !shown;
        }
        cx.wake.set(outer.min(cx.wake.get()));
    }
}

#[allow(clippy::too_many_arguments)]
fn emit<'a>(
    cx: &Context,
    layer: &'a Layer,
    set: &LayerOverride,
    parent: Affine,
    opacity: f32,
    quality: Quality,
    frame: &mut Frame<'a>,
    canvas: [f32; 2],
) {
    let t = &layer.transform;
    let view = frame.view;
    let base = layer.anchor.map_or([0.0; 2], |a| {
        let [fx, fy] = a.fraction();
        [view[0] + fx * view[2], view[1] + fy * view[3]]
    });
    let free = layer.anchor.is_some() || matches!(layer.kind, Kind::Group { .. } | Kind::Adjustment {});
    let center = if free { [0.0; 2] } else { [canvas[0] / 2.0, canvas[1] / 2.0] };
    let position = set.position.unwrap_or_else(|| t.position.as_ref().map_or(center, |p| p.get(cx).0));
    if layer.parallax != 0.0 && cx.pointer.moving() {
        cx.note(Wake::Frame);
    }
    let shift = cx.pointer.smooth.map(|s| s - 0.5);
    let at = [0, 1].map(|i| base[i] + position[i] + shift[i] * layer.parallax * canvas[i]);
    let local = Affine::translate(at)
        .then(Affine::rotate(set.rotation.unwrap_or_else(|| t.rotation.get(cx))))
        .then(Affine::skew(t.skew.get(cx).0))
        .then(Affine::scale(set.scale.unwrap_or_else(|| t.scale.get(cx).0)));
    let origin = t.origin.or(layer.anchor.map(|a| a.fraction())).unwrap_or([0.5; 2]);
    let content = content(cx, layer, set, canvas);
    let start = frame.effects.len();
    for effect in &layer.effects {
        let Some(d) = descriptor(&effect.effect).filter(|_| effect.enabled.get(cx)) else { continue };
        let Some(scale) = quality.effect_scale(d.class(), effect.optional) else { continue };
        if d.animates() {
            cx.note(Wake::Frame);
        }
        let slots = d.pack(|id| effect.params.get(id).map(|v| v.get(cx)));
        let reach = d.reach.as_ref().and_then(|r| d.param(r)).map_or(0.0, |i| slots[i][0].abs());
        frame.effects.push(EffectDraw { descriptor: d, slots, mask: effect.mask.as_ref(), scale, reach });
    }
    let effects = start..frame.effects.len();
    let group = matches!(layer.kind, Kind::Group { .. });
    let isolate = !effects.is_empty()
        || !layer.blend.fixed()
        || layer.mask.is_some()
        || layer.cache
        || (group && opacity < 1.0);
    let index = frame.draws.len();
    let world = parent.then(local);
    let (blend, mask) = (layer.blend, layer.mask.as_ref());
    let draw = Draw {
        layer,
        end: index + 1,
        world,
        origin,
        opacity,
        blend,
        mask,
        effects,
        content,
        dynamic: false,
        isolate,
        hidden: false,
    };
    frame.draws.push(draw);
    walk(cx, layer.children(), world, quality, frame, canvas);
    let moving =
        matches!(layer.kind, Kind::Video { .. } | Kind::Particles { .. }) || cx.wake.get() == Wake::Frame;
    let children = frame.draws[index + 1..].iter().any(|d| d.dynamic);
    let end = frame.draws.len();
    let draw = &mut frame.draws[index];
    (draw.end, draw.dynamic) = (end, moving || children);
}

fn content<'a>(cx: &Context, layer: &'a Layer, set: &LayerOverride, canvas: [f32; 2]) -> Content<'a> {
    let tint = |c: Color| set.color.map_or(c, Color);
    match &layer.kind {
        Kind::Group { clip, .. } => Content::Group { clip: *clip },
        Kind::Image { image, size } => {
            let asset = match image {
                crate::Value::Fixed(id) => Cow::Borrowed(id.as_str()),
                bound => Cow::Owned(bound.get(cx)),
            };
            Content::Image { asset, size: *size }
        }
        Kind::Video { video, size } => Content::Video { asset: video, size: *size },
        Kind::Text { text, font, size, color, align } => {
            let text = match (&set.text, text) {
                (Some(t), _) => Cow::Owned(t.clone()),
                (None, crate::Value::Fixed(t)) => Cow::Borrowed(t.as_str()),
                (None, bound) => Cow::Owned(bound.get(cx)),
            };
            Content::Text {
                text,
                font: font.as_deref(),
                size: *size,
                color: tint(color.get(cx)),
                align: *align,
            }
        }
        Kind::Shape { shape, fill, gradient, stroke } => Content::Shape {
            shape: *shape,
            fill: fill.as_ref().map(|f| tint(f.get(cx))),
            gradient: gradient.as_ref(),
            stroke: stroke.as_ref().map(|s| (s.color.get(cx), s.width)),
        },
        Kind::Spectrum(spectrum) => {
            if !cx.audio.silent {
                cx.note(Wake::Frame);
            }
            Content::Spectrum { spectrum, color: tint(spectrum.color.get(cx)) }
        }
        Kind::Solid { color, size } => {
            Content::Solid { color: tint(color.get(cx)), size: size.unwrap_or(canvas) }
        }
        Kind::Particles { system } => Content::Particles { system },
        Kind::Adjustment {} => Content::Adjustment,
    }
}
