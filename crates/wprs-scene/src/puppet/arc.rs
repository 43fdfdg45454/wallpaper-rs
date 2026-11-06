use super::limb::{angle, gap, length, wrap};
use std::f32::consts::{PI, TAU};

fn end(lengths: &[f32], turn: f32) -> [f32; 2] {
    let mut at = [0.0f32; 2];
    for (i, l) in lengths.iter().enumerate() {
        let (s, c) = (turn * i as f32).sin_cos();
        at = [at[0] + l * c, at[1] + l * s];
    }
    at
}

pub(super) fn directions(points: &[[f32; 2]], target: [f32; 2], side: f32) -> Vec<f32> {
    let lengths: Vec<f32> = points.windows(2).map(|w| length(gap(w[0], w[1]))).collect();
    let n = lengths.len();
    let d = length(gap(points[0], target));
    let (mut lo, mut hi) = (0.0f32, TAU / n as f32);
    if length(end(&lengths, hi)) >= d {
        lo = hi;
    }
    for _ in 0..32 {
        let mid = (lo + hi) / 2.0;
        if length(end(&lengths, mid)) > d { lo = mid } else { hi = mid }
    }
    let turn = -side * lo;
    let first = angle(gap(points[0], target)) - angle(end(&lengths, turn));
    (0..n).map(|i| first + turn * i as f32).collect()
}

pub(super) fn sided(bone: [f32; 2], before: [f32; 2], swing: f32, side: f32) -> f32 {
    let now = wrap(angle(bone) - angle(before));
    let next = now + swing;
    let next = if side > 0.0 { next.clamp(-PI, 0.0) } else { next.clamp(0.0, PI) };
    next - now
}

pub(super) fn kept(points: &[[f32; 2]], side: f32) -> bool {
    let bends = points.windows(3).map(|w| wrap(angle(gap(w[1], w[2])) - angle(gap(w[0], w[1]))));
    let (mut right, mut wrong) = (false, false);
    for t in bends {
        right |= -t * side > 1e-3;
        wrong |= t * side > 1e-3;
    }
    right && !wrong
}

pub(super) fn within(points: &[[f32; 2]], target: [f32; 2]) -> bool {
    let total: f32 = points.windows(2).map(|w| length(gap(w[0], w[1]))).sum();
    length(gap(points[0], target)) < total - 0.01
}
