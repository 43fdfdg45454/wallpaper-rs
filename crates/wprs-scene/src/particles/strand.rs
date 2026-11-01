use super::census::seen;
use super::{Particle, Sim};

pub const ROPE_POINTS: usize = 4097;
pub type Strand = [f32; 12];

struct Knot {
    pos: [f32; 2],
    size: f32,
    life: f32,
    color: [f32; 4],
}

fn knot(p: &Particle, deep: [f32; 4]) -> Knot {
    let (pos, k) = seen(p, deep);
    let color = [p.color[0], p.color[1], p.color[2], p.color[3] * p.look[0]];
    Knot { pos, size: p.look[1] * k, life: (p.age / p.life).min(1.0), color }
}

fn curve(k: [&Knot; 4], t: f32, smooth: bool) -> ([f32; 2], [f32; 2]) {
    let (t2, t3) = (t * t, t * t * t);
    let at = |i: usize| [k[0].pos[i], k[1].pos[i], k[2].pos[i], k[3].pos[i]];
    let one = |[p0, p1, p2, p3]: [f32; 4]| {
        if !smooth {
            return (p1 + (p2 - p1) * t, p2 - p1);
        }
        let (b, c, d) = (p2 - p0, 2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3, 3.0 * p1 - p0 - 3.0 * p2 + p3);
        (0.5 * (2.0 * p1 + b * t + c * t2 + d * t3), 0.5 * (b + c * (2.0 * t) + d * (3.0 * t2)))
    };
    let ((x, dx), (y, dy)) = (one(at(0)), one(at(1)));
    ([x, y], [dx, dy])
}

pub fn strung(particles: &[Particle], s: &Sim, order: &mut Vec<(u32, u32)>, out: &mut Vec<Strand>) {
    out.resize(ROPE_POINTS, [0.0; 12]);
    out.fill([0.0; 12]);
    let [r, _, shape, most] = s.rope;
    if r == 0 {
        return;
    }
    let (sub, smooth) = (shape & 255, shape >> 8 & 1 != 0);
    let next = s.ids[1].wrapping_add(s.total());
    order.clear();
    let alive = particles.iter().enumerate().filter(|(_, p)| p.age < p.life);
    order.extend(alive.filter_map(|(i, p)| {
        let back = next.wrapping_sub(p.id);
        (back >= 1 && back <= r).then_some((back, i as u32))
    }));
    order.sort_unstable();
    let per = sub + 1;
    let n = (order.len() as u32).min(most / per + 1);
    if n < 2 {
        return;
    }
    for q in 0..(n - 1) * per + 1 {
        let seg = (q / per).min(n - 2);
        let t = (q - seg * per) as f32 / per as f32;
        let pick = |i: i64| knot(&particles[order[i.clamp(0, n as i64 - 1) as usize].1 as usize], s.depth[2]);
        let s0 = i64::from(seg);
        let k = [pick(s0 - 1), pick(s0), pick(s0 + 1), pick(s0 + 2)];
        let (pos, tan) = curve([&k[0], &k[1], &k[2], &k[3]], t, smooth);
        let l = (tan[0] * tan[0] + tan[1] * tan[1]).sqrt();
        let normal = if l > 1e-6 { [-tan[1] / l, tan[0] / l] } else { [0.0, 1.0] };
        let mix = |a: f32, b: f32| a + (b - a) * t;
        let (a, b) = (&k[1], &k[2]);
        let c: [f32; 4] = std::array::from_fn(|i| mix(a.color[i], b.color[i]));
        let size = mix(a.size, b.size);
        let life = mix(a.life, b.life);
        out[q as usize] =
            [pos[0], pos[1], normal[0], normal[1], seg as f32 + t, size, life, 1.0, c[0], c[1], c[2], c[3]];
    }
}
