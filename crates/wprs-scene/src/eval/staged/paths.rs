use crate::eye::Eye;
use crate::lens::{Camera3d, CameraPath, FadeTo, PathOrder};
use crate::resolve::Context;

pub mod spline;

pub use spline::{at_time, period};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    pub eye: Eye,
    pub fade: f32,
    pub cross: Option<(Eye, f32)>,
    pub moving: bool,
}

fn mix(seed: u64) -> u64 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

pub fn sequence(count: usize, order: PathOrder, round: u64, out: &mut [usize]) {
    out.iter_mut().enumerate().for_each(|(i, o)| *o = i);
    if order == PathOrder::Random {
        for i in (1..count).rev() {
            let j = (mix(round * 64 + i as u64) % (i as u64 + 1)) as usize;
            out.swap(i, j);
        }
    }
}

pub(crate) fn flight(cx: &Context, cam: &Camera3d) -> Option<Flight> {
    play(&cam.paths, cam, cx.t, cam.fov.get(cx))
}

pub fn play(paths: &[CameraPath], cam: &Camera3d, t: f32, fov: f32) -> Option<Flight> {
    let count = paths.len().min(crate::lens::MAX_PATHS);
    if count == 0 {
        return None;
    }
    let base =
        Eye { at: [0.0; 3], center: [0.0; 3], up: cam.up.0, fov, near: cam.near, far: cam.far, aspect: 1.0 };
    let pose = |p: &CameraPath, u: f32| at_time(p, u, base);
    if count == 1 && !paths[0].closed {
        let p = &paths[0];
        let end = p.keys.last().map_or(0.0, |k| k.t);
        return Some(Flight { eye: pose(p, t.min(end)), fade: 0.0, cross: None, moving: t < end });
    }
    let lengths: [f32; crate::lens::MAX_PATHS] = std::array::from_fn(|i| paths.get(i).map_or(0.0, period));
    let total: f32 = lengths[..count].iter().sum::<f32>().max(1e-3);
    let round = (t / total).floor().max(0.0);
    let mut left = t - round * total;
    let mut order = [0usize; crate::lens::MAX_PATHS];
    sequence(count, cam.order, round as u64, &mut order[..count]);
    let mut k = 0;
    while k + 1 < count && left >= lengths[order[k]] {
        left -= lengths[order[k]];
        k += 1;
    }
    let (now, len) = (&paths[order[k]], lengths[order[k]].max(1e-3));
    let eye = pose(now, left);
    let Some(fade) = cam.fade.filter(|f| f.duration > 0.0 && count > 1) else {
        return Some(Flight { eye, fade: 0.0, cross: None, moving: true });
    };
    let next = match k + 1 < count {
        true => order[k + 1],
        false => {
            sequence(count, cam.order, round as u64 + 1, &mut order[..count]);
            order[0]
        }
    };
    let d = fade.duration.min(len);
    let smooth = |x: f32| x * x * (3.0 - 2.0 * x);
    let flight = match fade.to {
        FadeTo::Black => {
            let tail = ((left - (len - d * 0.5)) / (d * 0.5)).clamp(0.0, 1.0);
            let head = (1.0 - left / (d * 0.5)).clamp(0.0, 1.0);
            Flight { eye, fade: smooth(tail.max(head)), cross: None, moving: true }
        }
        FadeTo::Cross => {
            let f = ((left - (len - d)) / d).clamp(0.0, 1.0);
            let cross = (f > 0.0).then(|| (pose(&paths[next], 0.0), smooth(f)));
            Flight { eye, fade: 0.0, cross, moving: true }
        }
    };
    Some(flight)
}
