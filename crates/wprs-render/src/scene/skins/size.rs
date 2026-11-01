use wprs_scene::{Affine, Quality};

fn shrink(quality: Quality) -> f32 {
    match quality {
        Quality::Q0 => 0.5,
        Quality::Q1 => 0.75,
        _ => 1.0,
    }
}

pub(super) fn sized(size: [f32; 2], scale: f32, quality: Quality, edge: u32) -> (u32, u32) {
    let [w, h] = size.map(|v| ((v * scale * shrink(quality)).round() as u32).clamp(1, edge));
    (w, h)
}

pub(super) fn grown(world: Affine, size: [f32; 2], scale: f32, (pw, ph): (u32, u32)) -> f32 {
    let m = world.0;
    let reach = m[0].hypot(m[1]).max(m[2].hypot(m[3]));
    let room = (pw as f32 / (size[0] * scale)).max(ph as f32 / (size[1] * scale));
    let k = reach.min(room);
    if !k.is_finite() || k <= 1.0 { 1.0 } else { 2f32.powf((k.log2() * 2.0).ceil() / 2.0).min(room.max(1.0)) }
}
