use super::{EmitShape, MAX_COLLIDERS, MAX_EMITTERS, STEP, Surface, System};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct Sim {
    pub clock: [f32; 4],
    pub ids: [u32; 4],
    pub births: [u32; 4],
    pub counts: [u32; 4],
    pub shapes: [u32; 4],
    pub emitters: [[f32; 4]; 8],
    pub init: [[f32; 4]; 4],
    pub colors: [[f32; 4]; 2],
    pub forces: [[f32; 4]; 8],
    pub colliders: [[f32; 4]; 8],
}

impl Sim {
    pub(crate) fn of(s: &System, seed: u64, canvas: [u32; 2]) -> Sim {
        let mut b = Sim::default();
        let seed = s.seed ^ seed;
        b.clock = [STEP, 0.0, canvas[0] as f32, canvas[1] as f32];
        b.ids[0] = (seed ^ (seed >> 32)) as u32;
        let counts = [s.emitters.len(), s.forces.len(), s.collide.len()].map(|n| n as u32);
        b.counts = [s.capacity, counts[0], counts[1], counts[2]];
        for (e, em) in s.emitters.iter().take(MAX_EMITTERS).enumerate() {
            (b.shapes[e], b.emitters[2 * e]) = match em.shape {
                EmitShape::Point([x, y]) => (0, [x, y, 0.0, 0.0]),
                EmitShape::Box([p, q]) => (1, [p[0], p[1], q[0], q[1]]),
                EmitShape::Circle { center: [x, y], radius, inner } => (2, [x, y, radius, inner]),
                EmitShape::Line([p, q]) => (3, [p[0], p[1], q[0], q[1]]),
                EmitShape::Edge => (4, [0.0; 4]),
                EmitShape::Mask { .. } => (5, [0.0; 4]),
            };
        }
        let i = &s.init;
        b.init = [
            [i.life[0], i.life[1], i.speed[0], i.speed[1]],
            [i.angle[0], i.angle[1], i.size[0], i.size[1]],
            [i.rotation[0], i.rotation[1], i.spin[0], i.spin[1]],
            [i.frame[0], i.frame[1], 0.0, 0.0],
        ];
        b.colors = i.color.map(|c| c.0);
        for (k, c) in s.collide.iter().take(MAX_COLLIDERS).enumerate() {
            let ([kind, x, y, z], w) = match c.bounds {
                Surface::Canvas => ([1.0, 0.0, 0.0, 0.0], 0.0),
                Surface::Plane { point: [x, y], normal: [u, v] } => {
                    let l = u.hypot(v).max(1e-6);
                    ([2.0, x, y, u / l], v / l)
                }
                Surface::Circle { center: [x, y], radius } => ([3.0, x, y, radius], 0.0),
            };
            b.colliders[2 * k] = [kind, x, y, z];
            b.colliders[2 * k + 1] = [w, c.bounce, c.friction, f32::from(u8::from(c.kill))];
        }
        b
    }
}
