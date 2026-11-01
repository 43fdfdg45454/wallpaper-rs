use crate::look::Shake;
use crate::resolve::Context;
use crate::{Affine, Layer, Lean, Quality, Wake};

const OCTAVES: u32 = 4;

impl Context<'_> {
    pub(crate) fn lens(&self, quality: Quality) -> Affine {
        let Some(cam) = &self.scene.canvas.camera else { return Affine::IDENTITY };
        let half = self.canvas.map(|v| v / 2.0);
        let center = cam.center.as_ref().map_or(half, |c| c.get(self).0);
        let zoom = cam.zoom.get(self).clamp(0.1, 10.0);
        let moving = !self.still && quality > Quality::Q0;
        let shake = cam.shake.as_ref().filter(|_| moving).map_or([0.0; 2], |s| self.shake(s));
        Affine::translate([half[0] + shake[0], half[1] + shake[1]])
            .then(Affine::rotate(cam.rotation.get(self).clamp(-360.0, 360.0)))
            .then(Affine::scale([zoom; 2]))
            .then(Affine::translate([-center[0], -center[1]]))
    }

    fn shake(&self, s: &Shake) -> [f32; 2] {
        let amplitude = s.amplitude.get(self).clamp(0.0, 1000.0);
        let speed = s.speed.get(self).clamp(0.0, 60.0);
        if !s.enabled.get(self) || amplitude == 0.0 || speed == 0.0 {
            return [0.0; 2];
        }
        self.note(Wake::Frame);
        let rough = s.roughness.get(self).clamp(0.0, 1.0);
        [0, 1].map(|axis| amplitude * fbm(self.t * speed, axis, rough))
    }

    pub(crate) fn lean(&self, layer: &Layer, world: Affine, outer: Option<Lean>) -> Option<Lean> {
        let t = &layer.transform;
        if t.tilt.is_none() && t.z.is_none() {
            return outer;
        }
        let tilt = t.tilt.as_ref().map_or([0.0; 2], |v| v.get(self).0.map(|a| a.clamp(-89.0, 89.0)));
        let z = t.z.as_ref().map_or(0.0, |v| v.get(self).clamp(-1e5, 1e5));
        let back = self.camera.inverse();
        let pivot = back.apply([world.0[4], world.0[5]]);
        let view = (self.canvas.map(|v| v / 2.0), self.distance());
        let own = Lean::of(self.camera).then(Lean::tilted(pivot, tilt, z, view)).then(Lean::of(back));
        Some(outer.map_or(own, |o| o.then(own)))
    }

    fn distance(&self) -> f32 {
        let fov = || self.scene.camera3d.as_ref().map_or(45.0, |c| c.fov.get(self)).clamp(1.0, 179.0);
        let found = self.scene.canvas.perspective;
        found.unwrap_or_else(|| self.canvas[1] / 2.0 / (fov().to_radians() / 2.0).tan()).max(100.0)
    }
}

fn fbm(t: f32, axis: u32, rough: f32) -> f32 {
    let (mut sum, mut weight, mut total, mut f) = (0.0, 1.0, 0.0, 1.0);
    for octave in 0..OCTAVES {
        sum += weight * noise(t * f, axis * OCTAVES + octave);
        total += weight;
        (weight, f) = (weight * rough, f * 2.0);
    }
    sum / total
}

fn noise(x: f32, seed: u32) -> f32 {
    let hash = |n: f32| {
        let v = (n as i64 as u32).wrapping_mul(0x9e37_79b1) ^ seed.wrapping_mul(0x85eb_ca77);
        let v = (v ^ (v >> 15)).wrapping_mul(0x2c1b_3c6d);
        (v ^ (v >> 12)) as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let (i, k) = (x.floor(), x - x.floor());
    hash(i) + (hash(i + 1.0) - hash(i)) * k * k * (3.0 - 2.0 * k)
}
