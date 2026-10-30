use crate::input::LayerOverride;
use crate::resolve::Context;
use crate::wake::wakes;
use crate::{
    Affine, Align, Blend, Color, Descriptor, EffectUse, Gradient, Kind, Layer, Layout, MAX_PARAMS, Mask,
    Quality, Shape, Spectrum, Wake, fusion,
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

pub(crate) fn walk<'a>(
    cx: &Context<'a>,
    layers: &'a [Layer],
    parent: Affine,
    quality: Quality,
    frame: &mut Frame<'a>,
    canvas: [f32; 2],
) {
    let none = LayerOverride::default();
    for layer in layers.iter().filter(|l| l.flat()) {
        let set = cx.script.layers.get(&layer.id).unwrap_or(&none);
        let outer = cx.wake.replace(Wake::Idle);
        let place = cx.place(layer, parent);
        let (shown, at) = (place.visible && place.opacity > 0.0, frame.draws.len());
        if shown || cx.mattes.contains(&layer.id) {
            emit(cx, layer, set, place, quality, frame, canvas);
            frame.draws[at].hidden = !shown;
        }
        cx.wake.set(outer.min(cx.wake.get()));
    }
}

fn emit<'a>(
    cx: &Context<'a>,
    layer: &'a Layer,
    set: &LayerOverride,
    place: crate::kin::Place,
    quality: Quality,
    frame: &mut Frame<'a>,
    canvas: [f32; 2],
) {
    let (t, opacity) = (&layer.transform, place.opacity);
    let valign = match &layer.kind {
        Kind::Text(text) => text.valign.unwrap_or_default().fraction(),
        _ => 0.5,
    };
    let origin = t.origin.or(layer.anchor.map(|a| a.fraction())).unwrap_or([0.5, valign]);
    let content = content(cx, layer, set, canvas, quality);
    let playing = matches!(&content, Content::Image { asset, .. } if cx.playing.contains(asset.as_ref()));
    if playing || matches!(layer.kind, Kind::Video { .. }) {
        cx.pace.set(u32::MAX);
    }
    let start = frame.effects.len();
    let runs = if layer.effects.len() > 1 { fusion(&layer.effects) } else { Vec::new() };
    let mut paced = false;
    for (i, effect) in layer.effects.iter().enumerate() {
        let Some(d) = cx.scene.shader(&effect.effect).filter(|_| effect.enabled.get(cx)) else { continue };
        let Some(scale) = quality.effect_scale(d.class(), effect.optional) else { continue };
        let slots = d.pack(|id| d.setting(quality, id, effect.params.get(id).map(|v| v.get(cx))));
        let wake = wakes(d, &slots, cx);
        (paced, _) = (paced || matches!(wake, Wake::At(_)), cx.paced(wake, d.min_fps));
        let reach = d.reach.as_ref().and_then(|r| d.param(r)).map_or(0.0, |i| slots[i][0].abs());
        let (mask, scale) = (effect.mask.as_ref(), scale * d.scale_at(quality));
        let chain = runs.iter().find(|r| r.start == i).map_or(1, |r| r.len());
        let textures = d.params.iter().filter(|p| p.kind.asset());
        let textures = textures.filter_map(|p| match (effect.params.get(&p.id)?, p.id.as_str()) {
            (bound @ crate::Value::Bound(_), id) => match bound.get(cx) {
                crate::Param::Text(name) => Some((id, name)),
                _ => None,
            },
            _ => None,
        });
        let (textures, mask_texture) = (textures.collect(), masked(cx, mask));
        let mask_strength = strength(cx, mask);
        let follows = d.velocity.as_ref().and_then(|v| d.param(v)).is_some_and(|i| slots[i][0] != 0.0);
        let key = std::ptr::from_ref(effect) as usize;
        let velocity =
            if follows { cx.velocity(key, [place.world.0[4], place.world.0[5]]) } else { [0.0; 2] };
        if velocity != [0.0; 2] {
            cx.note(Wake::Frame);
        }
        let variant = d.variant(effect, |id| effect.params.get(id).map(|v| v.get(cx)));
        let draw = EffectDraw {
            variant,
            descriptor: d,
            effect,
            slots,
            mask,
            mask_texture,
            mask_strength,
            scale,
            reach,
            chain,
            textures,
            velocity,
        };
        frame.effects.push(draw);
    }
    let effects = start..frame.effects.len();
    let group = matches!(layer.kind, Kind::Group { .. });
    let painted = match &layer.kind {
        Kind::Image { color, .. } | Kind::Video { color, .. } | Kind::Particles { color, .. } => {
            color.as_ref()
        }
        Kind::Group { color, .. } => color.as_ref(),
        _ => None,
    };
    let tint = set.color.map(Color).filter(|_| painted.is_some() || group).or(painted.map(|c| c.get(cx)));
    let tint = tint.unwrap_or(Color([1.0; 4]));
    let brightness = layer.brightness.as_ref().map_or(1.0, |b| b.get(cx).clamp(0.0, 16.0));
    let tuned = match &layer.kind {
        Kind::Particles { system, render, .. } => cx.scene.particles.get(system).map(|s| {
            let glow = render.as_deref().unwrap_or(&s.render).brightness.as_ref();
            (crate::particles::tuned(cx, Some(layer), s), glow.map_or(1.0, |b| b.get(cx).clamp(0.0, 16.0)))
        }),
        _ => None,
    };
    let (opacity, tint, brightness) = match tuned {
        Some((t, glow)) => {
            let c = t.color.unwrap_or([1.0; 4]);
            let tint = Color(std::array::from_fn(|i| tint.0[i] * c[i]));
            (opacity * t.alpha, tint, brightness * t.brightness * glow)
        }
        None => (opacity, tint, brightness),
    };
    let toned = tint != Color([1.0; 4]) || brightness != 1.0;
    let isolate = !effects.is_empty()
        || !layer.blend.fixed()
        || layer.mask.is_some()
        || layer.cache
        || (group && (opacity < 1.0 || toned));
    let index = frame.draws.len();
    let world = place.world;
    let (blend, mask) = (layer.blend, layer.mask.as_ref());
    let mask_strength = strength(cx, mask);
    let draw = Draw {
        layer,
        end: index + 1,
        world,
        origin,
        opacity,
        blend,
        mask,
        mask_texture: masked(cx, mask),
        mask_strength,
        tint,
        brightness,
        effects,
        content,
        dynamic: false,
        isolate,
        hidden: false,
    };
    frame.draws.push(draw);
    walk(cx, layer.children(), world, quality, frame, canvas);
    let moving = paced
        || playing
        || matches!(layer.kind, Kind::Video { .. } | Kind::Particles { .. })
        || cx.wake.get() == Wake::Frame;
    let children = frame.draws[index + 1..].iter().any(|d| d.dynamic);
    let end = frame.draws.len();
    let draw = &mut frame.draws[index];
    (draw.end, draw.dynamic) = (end, moving || children);
}

fn content<'a>(
    cx: &Context,
    layer: &'a Layer,
    set: &LayerOverride,
    canvas: [f32; 2],
    quality: Quality,
) -> Content<'a> {
    let tint = |c: Color| set.color.map_or(c, Color);
    match &layer.kind {
        Kind::Group { clip, .. } => Content::Group { clip: *clip },
        Kind::Image { image, size, playback, .. } => {
            let asset = named(cx, image);
            let frame = crate::sheet::frame(cx, &asset, playback.as_ref(), quality);
            Content::Image { asset, size: *size, frame }
        }
        Kind::Video { video, size, .. } => Content::Video { asset: named(cx, video), size: *size },
        Kind::Text(t) => {
            let text = match (&set.text, &t.text) {
                (Some(t), _) => Cow::Owned(t.clone()),
                (None, crate::Value::Fixed(t)) => Cow::Borrowed(t.as_str()),
                (None, bound) => Cow::Owned(bound.get(cx)),
            };
            Content::Text {
                text,
                font: t.font.as_deref(),
                fallback: &t.fallback,
                size: t.size,
                color: tint(t.color.get(cx)),
                align: t.align,
                layout: Layout {
                    wrap: t.wrap,
                    line_height: t.line_height,
                    padding: t.padding.unwrap_or(0.0),
                    backing: t.background.as_ref().map(|b| (b.color.get(cx), b.padding, b.radius)),
                },
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
        Kind::Particles { system, .. } => {
            let index = cx.scene.particles.keys().position(|k| k == system).unwrap_or(usize::MAX);
            let flock = crate::particles::flock_of(cx.flocks, index, &layer.id);
            let size = cx
                .scene
                .particles
                .get(system)
                .map_or(1.0, |s| crate::particles::tuned(cx, Some(layer), s).size);
            Content::Particles { system, flock, size }
        }
        Kind::Adjustment { .. } | Kind::Sound(_) | Kind::Light(_) | Kind::Mesh(_) | Kind::Model(_) => {
            Content::Adjustment
        }
    }
}

fn strength(cx: &Context, mask: Option<&Mask>) -> f32 {
    mask.and_then(|m| m.strength.as_ref()).map_or(1.0, |s| s.get(cx).clamp(0.0, 1.0))
}

fn masked<'a>(cx: &Context, mask: Option<&'a Mask>) -> Option<Cow<'a, str>> {
    mask?.texture.as_ref().map(|t| named(cx, t))
}

fn named<'a>(cx: &Context, value: &'a crate::Value<String>) -> Cow<'a, str> {
    match value {
        crate::Value::Fixed(id) => Cow::Borrowed(id.as_str()),
        bound => Cow::Owned(bound.get(cx)),
    }
}
