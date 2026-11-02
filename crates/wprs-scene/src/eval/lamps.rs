use crate::light::{Beam, Light, MAX_LIGHTS};
use crate::resolve::Context;
use crate::{Affine, Color, Kind, Layer, Lean, Lighting, Lit, Quality, Value, Wake};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub enum Picture<'a> {
    Asset(Cow<'a, str>),
    Layer(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lamp<'a> {
    pub layer: &'a Layer,
    pub beam: Beam,
    pub world: Affine,
    pub tilt: f32,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    pub z: f32,
    pub cone: [f32; 2],
    pub length: f32,
    pub air: Option<[f32; 3]>,
    pub picture: Option<Picture<'a>>,
    pub slot: usize,
}

impl Lamp<'_> {
    pub fn at(&self) -> [f32; 2] {
        [self.world.0[4], self.world.0[5]]
    }

    pub fn heading(&self) -> [f32; 3] {
        let [a, b, c, d, ..] = self.world.0;
        let (x, y) = if self.beam == Beam::Tube { (a, b) } else { (c, d) };
        let long = x.hypot(y).max(1e-6);
        let flat = self.tilt.to_radians().cos();
        [x / long * flat, y / long * flat, -self.tilt.to_radians().sin()]
    }

    pub fn power(&self, distance: f32) -> f32 {
        let k = (1.0 - (distance / self.radius.max(1e-3)).powi(2)).clamp(0.0, 1.0);
        self.intensity * k * k
    }
}

pub(crate) struct Gather<'a, 'b> {
    pub(crate) out: &'b mut Vec<Lamp<'a>>,
    pub(crate) draws: &'b [super::Draw<'a>],
    pub(crate) next: usize,
}

pub(crate) fn lamps<'a>(
    cx: &Context<'a>,
    layers: &'a [Layer],
    outer: (Affine, Option<Lean>),
    (shown, lit): (bool, bool),
    quality: Quality,
    g: &mut Gather<'a, '_>,
) {
    for layer in layers {
        if g.draws.get(g.next).is_some_and(|d| std::ptr::eq(d.layer, layer)) {
            g.next += 1;
        }
        match &layer.kind {
            Kind::Group { layers, .. } => {
                let p = cx.place(layer, outer);
                lamps(cx, layers, (p.world, p.lean), (shown && p.visible, lit), quality, g);
            }
            Kind::Light(l) if g.out.len() < MAX_LIGHTS => {
                let before = cx.wake.replace(Wake::Idle);
                let found = lamp(cx, layer, l, outer, quality).filter(|_| shown);
                let seen = lit || l.volumetric.is_some();
                let wake = if quality == Quality::Q0 || !seen { before } else { before.min(cx.wake.get()) };
                cx.wake.set(wake);
                let slot = g.next;
                g.out.extend(found.map(|f| Lamp { slot, ..f }));
            }
            _ => {}
        }
    }
}

fn lamp<'a>(
    cx: &Context<'a>,
    layer: &'a Layer,
    l: &'a Light,
    outer: (Affine, Option<Lean>),
    quality: Quality,
) -> Option<Lamp<'a>> {
    let place = cx.place(layer, outer);
    let tilt = layer.transform.tilt.as_ref().map_or(0.0, |t| t.get(cx).0[0].clamp(-89.0, 89.0));
    let cone = l.cone.unwrap_or([30.0, 60.0]);
    let picture = l.texture.as_ref().filter(|_| l.light == Beam::Spot).map(|t| match t {
        Value::Bound(b) if b.layer.is_some() => Picture::Layer(b.layer.as_deref().unwrap_or_default()),
        t => Picture::Asset(super::shown::named(cx, t)),
    });
    if let Some(Picture::Asset(id)) = &picture
        && !cx.scene.pools.contains_key(id.as_ref())
        && cx.rolls(id, false)
    {
        cx.paced(Wake::Frame, None);
    }
    place.visible.then_some(Lamp {
        layer,
        beam: l.light,
        world: place.world,
        tilt,
        color: l.color.get(cx),
        intensity: l.intensity.get(cx).clamp(0.0, 100.0),
        radius: l.radius.get(cx).clamp(1.0, 1e5),
        z: l.z.get(cx).clamp(-1e5, 1e5),
        cone: [cone[0].clamp(0.0, 180.0), cone[1].clamp(0.0, 180.0).max(cone[0])],
        length: l.length.unwrap_or(256.0).clamp(1.0, 1e5),
        air: l
            .volumetric
            .filter(|_| quality != Quality::Q0 && matches!(l.light, Beam::Point | Beam::Spot))
            .map(|v| [v.density.clamp(0.0, 1.0), v.samples.clamp(4, 64) as f32, v.shafts.clamp(0.0, 1.0)]),
        picture,
        slot: 0,
    })
}

pub(crate) fn seen(scene: &crate::Scene) -> bool {
    fn takes(l: &Layer) -> bool {
        let sprites = matches!(&l.kind, Kind::Particles { render: Some(r), .. } if r.lit);
        l.lighting.is_some() || sprites || l.children().iter().any(takes)
    }
    let systems = scene.particles.values().any(|s| s.render.lit);
    systems || scene.layers.iter().chain(scene.templates.values()).any(takes)
}

pub(crate) fn lighting<'a>(cx: &Context<'a>, l: &'a Lighting) -> Lit<'a> {
    let unit = |v: &crate::Value<f32>| v.get(cx).clamp(0.0, 1.0);
    Lit {
        strength: unit(&l.strength),
        roughness: unit(&l.roughness),
        reflection: unit(&l.reflection),
        emissive: l.emissive.get(cx).clamp(0.0, 16.0),
        normal: l.normal.as_deref(),
        env: l.env.as_deref(),
        roughness_map: l.roughness_map.as_deref(),
        emissive_map: l.emissive_map.as_deref(),
    }
}
