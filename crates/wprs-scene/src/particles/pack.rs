use super::{EmitShape, Inheritable, MAX_EMITTERS2, Moment, Part, STEP, Spread, System};
use bytemuck::{Pod, Zeroable};

pub const PARTICLE_EVENTS: u32 = 1024;
pub const MAX_HATCH: u32 = 1000;
pub const EVENT: usize = 16;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Sim {
    pub clock: [f32; 4],
    pub ids: [u32; 4],
    pub counts: [u32; 4],
    pub mode: [u32; 4],
    pub links: [u32; 4],
    pub births: [u32; 8],
    pub shapes: [u32; 8],
    pub rates: [f32; 8],
    pub emitters: [[f32; 4]; 32],
    pub init: [[f32; 4]; 10],
    pub colors: [[f32; 4]; 10],
    pub tones: [[f32; 4]; 3],
    pub forces: [[f32; 4]; 64],
    pub colliders: [[f32; 4]; 16],
    pub hatch: [[f32; 4]; 2],
}

impl Default for Sim {
    fn default() -> Self {
        Zeroable::zeroed()
    }
}

fn ends(s: &Spread<f32>) -> [f32; 2] {
    s.ends
}

fn exp(s: &Spread<f32>) -> f32 {
    s.exponent.unwrap_or(1.0)
}

impl Sim {
    pub fn total(&self) -> u32 {
        self.births[MAX_EMITTERS2 - 1]
    }

    pub(crate) fn of(s: &System, part: &Part, host: Option<&System>, seed: u64, canvas: [u32; 2]) -> Sim {
        let mut b = Sim::default();
        let seed = s.seed ^ seed ^ (part.depth as u64 * 0x9e37_79b9 + part.link as u64);
        b.clock = [STEP, 0.0, canvas[0] as f32, canvas[1] as f32];
        b.ids[0] = (seed ^ (seed >> 32)) as u32;
        let counts = [s.emitters.len().min(MAX_EMITTERS2), s.forces.len().min(16), s.collide.len().min(8)];
        let [e, f, c] = counts.map(|n| n as u32);
        b.counts = [s.capacity, e, f, c];
        let makes = |m: Moment| s.children.iter().any(|c| c.on == m);
        let bits = [(Moment::Spawn, 1), (Moment::Death, 2), (Moment::Follow, 4)];
        b.mode[1] = bits.iter().filter(|(m, _)| makes(*m)).map(|(_, b)| b).sum();
        b.mode[2] = u32::MAX;
        b.hatch[1] = [1.0, 0.0, 0.0, 0.0];
        let link = host.zip(part.on).and_then(|(h, _)| h.children.get(part.link));
        if let Some(c) = link {
            b.mode[0] = match c.on {
                Moment::Spawn => 1,
                Moment::Death => 2,
                Moment::Follow => 3,
                Moment::Static => 0,
            };
            b.mode[2] = c.max.unwrap_or(u32::MAX);
            let bit = |i: Inheritable, v: u32| if c.inherit.contains(&i) { v } else { 0 };
            let inherit = bit(Inheritable::Color, 1)
                + bit(Inheritable::Size, 2)
                + bit(Inheritable::Alpha, 4)
                + bit(Inheritable::Velocity, 8);
            let follow = f32::from(u8::from(c.follow_inherit));
            b.hatch[0] = [c.probability, part.link as f32, inherit as f32, follow];
            let stay = f32::from(u8::from(s.space == super::Placement::World));
            b.hatch[1] = [c.scale, stay, 0.0, 0.0];
        }
        for (e, em) in s.emitters.iter().take(MAX_EMITTERS2).enumerate() {
            let (kind, a, x) = match &em.shape {
                EmitShape::Point([x, y]) => (0, [*x, *y, 0.0, 0.0], [0.0; 4]),
                EmitShape::Box(a) => {
                    let [w, h] = a.inner.unwrap_or_default();
                    (1, [a.from[0], a.from[1], a.to[0], a.to[1]], [w, h, 0.0, 0.0])
                }
                EmitShape::Circle { center: [x, y], radius, inner, scale, sign, .. } => {
                    let [sx, sy] = scale.unwrap_or([1.0; 2]);
                    let [gx, gy] = sign.unwrap_or_default().map(f32::from);
                    (2, [*x, *y, radius.0, inner.0], [sx, sy, gx, gy])
                }
                EmitShape::Line([p, q]) => (3, [p[0], p[1], q[0], q[1]], [0.0; 4]),
                EmitShape::Edge => (4, [0.0; 4], [0.0; 4]),
                EmitShape::Mask { .. } => (5, [0.0; 4], [0.0; 4]),
                EmitShape::Layer(_) | EmitShape::Around(_) | EmitShape::Between(_) => (0, [0.0; 4], [0.0; 4]),
            };
            (b.shapes[e], b.emitters[4 * e], b.emitters[4 * e + 1]) = (kind, a, x);
            b.emitters[4 * e + 2] = [1.0, 0.0, 0.0, 1.0];
        }
        b.fill(s);
        b
    }

    fn fill(&mut self, s: &System) {
        let i = &s.init;
        let [life, speed, angle, size, rot, spin, frame] =
            [&i.life, &i.speed, &i.angle, &i.size, &i.rotation, &i.spin, &i.frame].map(ends);
        let alpha = i.alpha.as_ref().map_or([-1.0; 2], ends);
        let radial = i.radial.as_ref().map_or([0.0; 2], ends);
        let carry = i.inherit.map_or([0.0; 2], |c| c.speed);
        let [v0, v1] = i.velocity.as_ref().map_or([[0.0; 2]; 2], |v| v.ends);
        let one = |s: &Option<Spread<f32>>| s.as_ref().map_or(1.0, exp);
        self.init = [
            [life[0], life[1], speed[0], speed[1]],
            [angle[0], angle[1], size[0], size[1]],
            [rot[0], rot[1], spin[0], spin[1]],
            [frame[0], frame[1], alpha[0], alpha[1]],
            [radial[0], radial[1], carry[0], carry[1]],
            [v0[0], v0[1], v1[0], v1[1]],
            [exp(&i.life), exp(&i.speed), exp(&i.angle), exp(&i.size)],
            [exp(&i.rotation), exp(&i.spin), exp(&i.frame), one(&i.alpha)],
            [one(&i.radial), i.velocity.as_ref().and_then(|v| v.exponent).unwrap_or(1.0), 1.0, 0.0],
            [0.0, 0.0, f32::from(u8::from(i.velocity.is_some())), 0.0],
        ];
        self.init[8][2] = i.color.exponent.unwrap_or(1.0);
        self.colors[..2].copy_from_slice(&i.color.ends.map(|c| c.0));
        if let Some(h) = &i.hsv {
            self.init[9][3] = 1.0;
            self.tones[0] = [h.hue[0], h.hue[1], h.sat[0], h.sat[1]];
            self.tones[1] = [h.val[0], h.val[1], h.steps.unwrap_or(0) as f32, 0.0];
        }
        if let Some(p) = &i.palette {
            self.init[9][3] = 2.0;
            let n = p.colors.len().min(self.colors.len());
            self.colors[..n].iter_mut().zip(&p.colors).for_each(|(c, p)| *c = p.0);
            self.tones[1][3] = n as f32;
            self.tones[2] = [p.noise[0], p.noise[1], p.noise[2], 0.0];
        }
    }
}
