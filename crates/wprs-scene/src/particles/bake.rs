use super::{EmitShape, MAX_EMITTERS, Sim};
use crate::{Kind, Scene, Tier};

pub const CPU_PARTICLES: u32 = 5_000;
pub const MAX_POINTS: usize = 4096;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bake {
    pub alloc: u32,
    pub size: f32,
    pub trail: u32,
    pub points: Vec<[f32; 2]>,
    ranges: [[f32; 4]; MAX_EMITTERS],
}

impl Bake {
    pub fn new<'a>(
        scene: &Scene,
        id: &str,
        tier: Tier,
        cpu: bool,
        image: impl Fn(&str) -> Option<(u32, u32, &'a [u8])>,
    ) -> Option<Bake> {
        let s = scene.particles.get(id)?;
        let draws = |id: &str| {
            let draws =
                scene.layers().filter(|l| matches!(&l.kind, Kind::Particles { system } if system == id));
            draws.count().max(1) as u64
        };
        let total: u64 = scene.particles.iter().map(|(id, s)| u64::from(s.capacity) * draws(id)).sum();
        let cap = u64::from(if cpu { tier.particles().min(CPU_PARTICLES) } else { tier.particles() });
        let alloc =
            (u64::from(s.capacity) * cap / total.max(cap)).clamp(1, u64::from(s.capacity.max(1))) as u32;
        let spread =
            s.overdraw(scene.canvas.size) * alloc as f32 / s.capacity.max(1) as f32 * draws(id) as f32;
        let size = (tier.overdraw() / spread.max(1e-9)).sqrt().min(1.0);
        let mut bake = Bake { alloc, size, trail: tier.trail(), ..Bake::default() };
        let [w, h] = scene.canvas.size.map(|v| v as f32);
        for (e, em) in s.emitters.iter().take(MAX_EMITTERS).enumerate() {
            let EmitShape::Mask { texture, threshold } = &em.shape else { continue };
            let Some((iw, ih, rgba)) = image(texture) else { continue };
            let stride = ((iw as f32 * ih as f32 / MAX_POINTS as f32).sqrt().ceil() as usize).max(1);
            let start = bake.points.len();
            let at = |v: usize, n: u32, side: f32| (v as f32 + stride as f32 / 2.0) / n as f32 * side;
            for y in (0..ih as usize).step_by(stride) {
                for x in (0..iw as usize).step_by(stride) {
                    let alpha = rgba.get((y * iw as usize + x) * 4 + 3).copied().unwrap_or(0);
                    if f32::from(alpha) / 255.0 > *threshold {
                        bake.points.push([at(x, iw, w), at(y, ih, h)]);
                    }
                }
            }
            let cell = [stride as f32 / iw as f32 * w, stride as f32 / ih as f32 * h];
            bake.ranges[e] = [start as f32, (bake.points.len() - start) as f32, cell[0], cell[1]];
        }
        Some(bake)
    }

    pub fn apply(&self, sim: &Sim) -> Sim {
        let mut out = *sim;
        out.counts[0] = self.alloc;
        out.ids[3] = sim.ids[3].min(self.alloc);
        let (mut total, mut last) = (0u32, 0);
        for e in 0..MAX_EMITTERS {
            let n = sim.births[e].saturating_sub(last);
            last = sim.births[e];
            let mask = out.shapes[e] == 5;
            if mask {
                out.emitters[2 * e] = self.ranges[e];
            }
            total = total.saturating_add(if mask && self.ranges[e][1] < 1.0 { 0 } else { n }).min(out.ids[3]);
            out.births[e] = total;
        }
        out
    }
}
