use crate::eval::Content;
use crate::input::LayerOverride;
use crate::resolve::Context;
use crate::{Color, Kind, Layer, Layout, Mask, Quality, Wake};
use std::borrow::Cow;

pub(crate) fn content<'a>(
    cx: &Context<'a>,
    layer: &'a Layer,
    set: &'a LayerOverride,
    canvas: [f32; 2],
    quality: Quality,
) -> Content<'a> {
    let tint = |c: Color| set.color.map_or(c, Color);
    let boxed = |size: Option<[f32; 2]>| match set.size {
        Some(crate::Extent::Box(b)) => Some(b),
        _ => size,
    };
    match &layer.kind {
        Kind::Group { clip, .. } => Content::Group { clip: *clip },
        Kind::Image { image, size, playback, .. } => {
            let asset = set.file.as_deref().map_or_else(|| named(cx, image), Cow::Borrowed);
            let frame = crate::sheet::frame(cx, &asset, playback.as_ref(), quality);
            Content::Image { asset, size: boxed(*size), frame }
        }
        Kind::Video { video, size, .. } => {
            let asset = set.file.as_deref().map_or_else(|| named(cx, video), Cow::Borrowed);
            Content::Video { asset, size: boxed(*size) }
        }
        Kind::Text(t) => {
            let ink = t.ink(cx, quality);
            let text = match (&set.text, &t.text) {
                (Some(t), _) => Cow::Owned(t.clone()),
                (None, crate::Value::Fixed(t)) => Cow::Borrowed(t.as_str()),
                (None, bound) => Cow::Owned(bound.get(cx)),
            };
            Content::Text {
                text,
                font: set.font.as_deref().or(t.font.as_deref()),
                fallback: &t.fallback,
                size: match set.size {
                    Some(crate::Extent::Points(p)) => p,
                    _ => t.size,
                },
                color: tint(t.color.get(cx)),
                align: set.align.unwrap_or(t.align),
                layout: Layout {
                    wrap: t.wrap,
                    line_height: t.line_height,
                    padding: t.padding.unwrap_or(0.0),
                    backing: t.background.as_ref().map(|b| (b.color.get(cx), b.padding, b.radius)),
                    spacing: t.spacing(cx),
                    reach: ink.map_or(0.0, |i| i.reach),
                },
                ink,
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
            Content::Solid { color: tint(color.get(cx)), size: boxed(*size).unwrap_or(canvas) }
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
        Kind::Mesh(m) => {
            let puppet = cx.puppets.iter().find(|p| p.layer == layer.id);
            if let Some(p) = puppet {
                cx.paced(if p.dirty { Wake::Frame } else { p.wake }, (quality == Quality::Q0).then_some(30));
            }
            let pose = puppet.map_or(0, |p| p.posed.generation);
            let texture = m.texture.as_ref().map(|t| named(cx, t));
            Content::Mesh {
                layer: &layer.id,
                size: boxed(m.size).unwrap_or(canvas),
                pose,
                texture,
                relief: None,
            }
        }
        Kind::Adjustment { .. } | Kind::Sound(_) | Kind::Light(_) | Kind::Model(_) => Content::Adjustment,
    }
}

pub(crate) fn strength(cx: &Context, mask: Option<&Mask>) -> f32 {
    mask.and_then(|m| m.strength.as_ref()).map_or(1.0, |s| s.get(cx).clamp(0.0, 1.0))
}

pub(crate) fn masked<'a>(cx: &Context<'a>, mask: Option<&'a Mask>) -> Option<Cow<'a, str>> {
    mask?.texture.as_ref().map(|t| named(cx, t))
}

pub(crate) fn named<'a>(cx: &Context<'a>, value: &'a crate::Value<String>) -> Cow<'a, str> {
    let plain = |b: &crate::Binding<String>| !b.tracked() && b.keys.is_none() && b.base.is_none();
    match value {
        crate::Value::Fixed(id) => Cow::Borrowed(id.as_str()),
        crate::Value::Bound(b) if plain(b) && b.map.is_none() => {
            match b.prop.as_ref().map(|id| cx.props.get(id)) {
                Some(Some(serde_json::Value::String(id))) => Cow::Borrowed(id.as_str()),
                Some(None | Some(serde_json::Value::Null)) => Cow::Borrowed("null"),
                _ => Cow::Owned(value.get(cx)),
            }
        }
        bound => Cow::Owned(bound.get(cx)),
    }
}
