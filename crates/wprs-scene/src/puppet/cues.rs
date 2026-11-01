use crate::{AnimClip, Fired, Loop, MAX_FIRED};

pub(super) fn period(repeat: Loop, d: f64) -> f64 {
    match repeat {
        Loop::Once => f64::INFINITY,
        Loop::Repeat => d,
        Loop::Pingpong => 2.0 * d,
    }
}

pub(super) fn crossed(
    (layer, clip): (&str, &AnimClip),
    d: f64,
    (from, to, fresh): (f64, f64, bool),
    speed: f32,
    out: &mut Vec<(Fired, f32)>,
) {
    if d <= 0.0 || clip.events.is_empty() || to < from {
        return;
    }
    let (start, period) = (out.len(), period(clip.repeat, d));
    for e in &clip.events {
        let t = f64::from(e.t);
        let mirror = (clip.repeat == Loop::Pingpong && t > 0.0 && t < d).then_some(2.0 * d - t);
        for first in [Some(t), mirror].into_iter().flatten() {
            let laps = ((from - first) / period).floor().max(0.0);
            let mut x = if period.is_finite() { first + laps * period } else { first };
            while x <= to && out.len() < MAX_FIRED {
                if x > from || (fresh && x >= from) {
                    let late = ((to - x) / f64::from(speed.max(1e-3))) as f32;
                    let timeline = format!("{layer}#{}", clip.id);
                    out.push((Fired { timeline, name: Some(e.name.clone()), t: e.t }, late));
                }
                x += period;
            }
        }
    }
    out[start..].sort_by(|a, b| b.1.total_cmp(&a.1));
}
