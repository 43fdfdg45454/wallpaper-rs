use crate::eye::{Eye, unit3};
use crate::lens::{CameraKey, CameraPath};

type V = [f32; 3];

pub fn period(p: &CameraPath) -> f32 {
    let (Some(first), Some(last)) = (p.keys.first(), p.keys.last()) else { return 0.0 };
    let n = p.keys.len() as f32;
    match p.closed && p.keys.len() > 1 {
        true => (last.t - first.t) * n / (n - 1.0),
        false => last.t,
    }
}

fn lerp(a: V, b: V, x: f32) -> V {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * x)
}

fn gap(a: V, b: V) -> f32 {
    (0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f32>().sqrt().sqrt().max(1e-4)
}

pub fn centripetal([p0, p1, p2, p3]: [V; 4], s: f32) -> V {
    let t1 = gap(p0, p1);
    let t2 = t1 + gap(p1, p2);
    let t3 = t2 + gap(p2, p3);
    let tt = t1 + s * (t2 - t1);
    let a1 = lerp(p0, p1, tt / t1);
    let a2 = lerp(p1, p2, (tt - t1) / (t2 - t1));
    let a3 = lerp(p2, p3, (tt - t2) / (t3 - t2));
    let b1 = lerp(a1, a2, tt / t2);
    let b2 = lerp(a2, a3, (tt - t1) / (t3 - t1));
    lerp(b1, b2, (tt - t1) / (t2 - t1))
}

fn pick(keys: &[CameraKey], j: isize, closed: bool, get: &dyn Fn(&CameraKey) -> V) -> V {
    let n = keys.len() as isize;
    match closed {
        true => get(&keys[j.rem_euclid(n) as usize]),
        false if j < 0 => {
            let (a, b) = (get(&keys[0]), get(&keys[1.min(n as usize - 1)]));
            std::array::from_fn(|i| 2.0 * a[i] - b[i])
        }
        false if j >= n => {
            let (a, b) = (get(&keys[n as usize - 1]), get(&keys[(n - 2).max(0) as usize]));
            std::array::from_fn(|i| 2.0 * a[i] - b[i])
        }
        false => get(&keys[j as usize]),
    }
}

pub fn at_time(p: &CameraPath, u: f32, base: Eye) -> Eye {
    let keys = &p.keys;
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else { return base };
    let closed = p.closed && keys.len() > 1;
    let span = if closed { period(p) } else { last.t - first.t };
    let local = match closed {
        true => u.rem_euclid(span.max(1e-6)),
        false => (u - first.t).clamp(0.0, span.max(0.0)),
    };
    let f = p.ease.map_or(local / span.max(1e-6), |e| e.apply((local / span.max(1e-6)).clamp(0.0, 1.0)));
    let t = first.t + f * span;
    let n = keys.len();
    let time = |j: usize| if j < n { keys[j].t } else { first.t + span };
    let i = (0..n).rev().find(|j| keys[*j].t <= t).unwrap_or(0);
    let i = if !closed && i + 1 >= n { n.saturating_sub(2) } else { i };
    let (a, b) = (time(i), time(i + 1));
    let s = if b > a { ((t - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
    let ring = |get: &dyn Fn(&CameraKey) -> V| -> [V; 4] {
        std::array::from_fn(|k| pick(keys, i as isize + k as isize - 1, closed, get))
    };
    if n == 1 {
        return Eye { at: first.eye.0, center: first.center.0, ..base };
    }
    let up = centripetal(ring(&|k| k.up.map_or(base.up, |u| u.0)), s);
    let fov = centripetal(ring(&|k| [k.fov.unwrap_or(base.fov), 0.0, 0.0]), s)[0];
    Eye {
        at: centripetal(ring(&|k| k.eye.0), s),
        center: centripetal(ring(&|k| k.center.0), s),
        up: unit3(up),
        fov: fov.clamp(1.0, 179.0),
        ..base
    }
}
