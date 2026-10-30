use super::Sim;
use super::motion::{bend, lerp, rand};
use super::spawn::KEY;

pub(super) fn tone(s: &Sim, id: u32) -> [f32; 4] {
    let r = |k: u32| rand(s.ids[0], id, k);
    let (i, t) = (s.init, s.tones);
    match i[9][3] as u32 {
        1 => {
            let steps = t[1][2];
            let u =
                if steps >= 1.0 { (r(KEY + 1) * steps).floor() / (steps - 1.0).max(1.0) } else { r(KEY + 1) };
            let hue = lerp(t[0][0], t[0][1], u.min(1.0));
            let rgb =
                hsv([hue / 360.0, lerp(t[0][2], t[0][3], r(KEY + 2)), lerp(t[1][0], t[1][1], r(KEY + 3))]);
            [rgb[0], rgb[1], rgb[2], 1.0]
        }
        2 => {
            let n = t[1][3].max(1.0);
            let c = s.colors[((r(KEY + 4) * n) as usize).min(n as usize - 1)];
            if t[2] == [0.0; 4] {
                return c;
            }
            let [h, sa, va] = unhsv([c[0], c[1], c[2]]);
            let jitter = |k: u32, a: f32| (r(KEY + k) - 0.5) * 2.0 * a;
            let h = h + jitter(5, t[2][0]) * 0.5;
            let sa = (sa + jitter(6, t[2][1])).clamp(0.0, 1.0);
            let va = (va + jitter(7, t[2][2])).clamp(0.0, 1.0);
            let rgb = hsv([h, sa, va]);
            [rgb[0], rgb[1], rgb[2], c[3]]
        }
        3 => {
            let (n, u) = ((t[1][3] as usize).clamp(1, 8), bend(r(9), i[8][2]));
            let at = |k: usize| t[if k < 4 { 0 } else { 2 }][k % 4];
            let mut c = s.colors[0];
            for k in (1..n).filter(|k| u >= at(k - 1)) {
                let f = ((u - at(k - 1)) / (at(k) - at(k - 1)).max(1e-6)).min(1.0);
                c = std::array::from_fn(|j| lerp(s.colors[k - 1][j], s.colors[k][j], f));
            }
            c
        }
        _ => {
            let u = bend(r(9), i[8][2]);
            std::array::from_fn(|j| lerp(s.colors[0][j], s.colors[1][j], u))
        }
    }
}

fn fract(x: f32) -> f32 {
    x - x.floor()
}

pub(super) fn hsv([h, s, v]: [f32; 3]) -> [f32; 3] {
    [1.0, 2.0 / 3.0, 1.0 / 3.0].map(|k| {
        let p = ((fract(h + k) * 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
        v * lerp(1.0, p, s)
    })
}

pub(super) fn unhsv([r, g, b]: [f32; 3]) -> [f32; 3] {
    let step = |edge: f32, x: f32| if x >= edge { 1.0 } else { 0.0 };
    let mix4 = |a: [f32; 4], b: [f32; 4], t: f32| std::array::from_fn::<f32, 4, _>(|i| lerp(a[i], b[i], t));
    let p = mix4([b, g, -1.0, 2.0 / 3.0], [g, b, 0.0, -1.0 / 3.0], step(b, g));
    let q = mix4([p[0], p[1], p[3], r], [r, p[1], p[2], p[0]], step(p[0], r));
    let d = q[0] - q[3].min(q[1]);
    [(q[2] + (q[3] - q[1]) / (6.0 * d + 1e-10)).abs(), d / (q[0] + 1e-10), q[0]]
}
