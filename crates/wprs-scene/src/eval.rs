use crate::input::LayerOverride;
use crate::resolve::Context;
use crate::wake::wakes;
use crate::{Affine, Color, Kind, Layer, Lean, Quality, Wake};
use std::borrow::Cow;

mod drawn;
pub(crate) mod lamps;
mod shown;

pub use drawn::{Content, Draw, EffectDraw, Frame, Lit};
use shown::{content, masked, strength};

pub(crate) fn walk<'a>(
    cx: &Context<'a>,
    layers: impl IntoIterator<Item = &'a Layer>,
    parent: (Affine, Option<Lean>),
    quality: Quality,
    frame: &mut Frame<'a>,
    canvas: [f32; 2],
) {
    for layer in layers.into_iter().filter(|l| l.flat()) {
        let set = cx.script.layers.get(&layer.id).unwrap_or(&crate::input::UNSET);
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
    set: &'a LayerOverride,
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
    let origin = set.origin.or(t.origin).or(layer.anchor.map(|a| a.fraction())).unwrap_or([0.5, valign]);
    let mut content = content(cx, layer, set, canvas, quality);
    if let (Content::Mesh { size, relief, .. }, Kind::Mesh(m)) = (&mut content, &layer.kind) {
        *relief = m.depth.map(|d| cx.relief(layer, &place, (origin, *size), d));
    }
    let playing = match &content {
        Content::Image { asset, .. } => cx.playing.contains(asset.as_ref()) || cx.rolls(asset, false),
        Content::Video { asset, .. } => !cx.scene.pools.contains_key(asset.as_ref()) && cx.rolls(asset, true),
        _ => false,
    };
    if playing {
        cx.pace.set(u32::MAX);
    }
    let start = frame.effects.len();
    let mut paced = false;
    let moves = playing || matches!(layer.kind, Kind::Particles { .. });
    let controls = cx.script.effects.get(&layer.id);
    for (i, effect) in layer.effects.iter().enumerate() {
        let ctl = controls.and_then(|list| list.iter().find(|c| c.at == i));
        let on = ctl.and_then(|c| c.enabled).unwrap_or_else(|| effect.enabled.get(cx));
        let Some(d) = cx.scene.shader(&effect.effect).filter(|_| on) else { continue };
        let kept = quality.effect_scale(d.class(), effect.optional);
        let frozen = kept.is_none() && d.frozen(quality);
        let Some(scale) = kept.or(frozen.then_some(0.25)) else { continue };
        let found = |id: &str| {
            let set = ctl.and_then(|c| c.params.get(id).cloned()).map(Cow::Owned);
            set.or_else(|| {
                effect.params.get(id).map(|v| match v {
                    crate::Value::Fixed(p) => Cow::Borrowed(p),
                    v => Cow::Owned(v.get(cx)),
                })
            })
        };
        let value = |id: &str| found(id).map(Cow::into_owned);
        let slots = d.pack(|id| d.setting(quality, id, found(id)));
        let key = std::ptr::from_ref(effect) as usize;
        let wake = match (frozen, &d.state) {
            (true, _) => Wake::Idle,
            (false, Some(_)) => {
                let stirring = moves || cx.wake.get() == Wake::Frame;
                wakes(d, &slots, cx, quality).min(cx.stirred(key, stirring, d.settle(&slots)))
            }
            (false, None) => wakes(d, &slots, cx, quality),
        };
        (paced, _) = (paced || matches!(wake, Wake::At(_)), cx.paced(wake, d.min_fps));
        let reach = d.reach.as_ref().and_then(|r| d.param(r)).map_or(0.0, |i| slots[i][0].abs());
        let (mask, scale) = (effect.mask.as_ref(), scale * d.scale_at(quality));
        let chain = crate::chain::runs(&layer.effects).find(|r| r.start == i).map_or(1, |r| r.len());
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
        let velocity =
            if follows { cx.velocity(key, [place.world.0[4], place.world.0[5]]) } else { [0.0; 2] };
        if velocity != [0.0; 2] {
            cx.note(Wake::Frame);
        }
        let variant = d.variant(effect, value);
        let draw = EffectDraw {
            variant,
            frozen,
            reset: ctl.map_or(0, |c| c.reset),
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
        Kind::Mesh(m) => m.color.as_ref(),
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
    let (world, lean) = (place.world, place.lean);
    let (blend, mask) = (layer.blend, layer.mask.as_ref());
    let mask_strength = strength(cx, mask);
    let draw = Draw {
        layer,
        end: index + 1,
        world,
        lean,
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
        lit: layer.lighting.as_ref().map(|l| lamps::lighting(cx, l)),
    };
    frame.draws.push(draw);
    walk(cx, layer.children(), (world, lean), quality, frame, canvas);
    let moving = paced || moves || cx.wake.get() == Wake::Frame;
    let children = frame.draws[index + 1..].iter().any(|d| d.dynamic);
    if children {
        layer.effects.iter().for_each(|e| _ = cx.stirred(std::ptr::from_ref(e) as usize, true, 0.0));
    }
    let end = frame.draws.len();
    let draw = &mut frame.draws[index];
    (draw.end, draw.dynamic) = (end, moving || children);
}
