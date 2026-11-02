use crate::light::{Beam, Light, MAX_LIGHTS};
use crate::resolve::Context;
use crate::{Affine, Color, Kind, Layer, Lean, Lighting, Lit, Quality, Wake};

#[derive(Debug, Clone, Copy, PartialEq)]
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

pub(crate) fn lamps<'a>(
    cx: &Context<'a>,
    layers: &'a [Layer],
    outer: (Affine, Option<Lean>),
    shown: bool,
    quality: Quality,
    out: &mut Vec<Lamp<'a>>,
) {
    for layer in layers {
        match &layer.kind {
            Kind::Group { layers, .. } => {
                let p = cx.place(layer, outer);
                lamps(cx, layers, (p.world, p.lean), shown && p.visible, quality, out);
            }
            Kind::Light(l) if out.len() < MAX_LIGHTS => {
                let before = cx.wake.replace(Wake::Idle);
                let found = lamp(cx, layer, l, outer).filter(|_| shown);
                let wake = if quality == Quality::Q0 { before } else { before.min(cx.wake.get()) };
                cx.wake.set(wake);
                out.extend(found);
            }
            _ => {}
        }
    }
}

fn lamp<'a>(
    cx: &Context<'a>,
    layer: &'a Layer,
    l: &Light,
    outer: (Affine, Option<Lean>),
) -> Option<Lamp<'a>> {
    let place = cx.place(layer, outer);
    let tilt = layer.transform.tilt.as_ref().map_or(0.0, |t| t.get(cx).0[0].clamp(-89.0, 89.0));
    let cone = l.cone.unwrap_or([30.0, 60.0]);
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
    })
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
