use super::maps::Asked;
use wgpu::TextureFormat as Format;
use wprs_scene::MAX_POINTS;

fn encode(c: f32) -> f32 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn texel(texels: &[u8], shot: Asked, i: usize) -> [f32; 4] {
    if shot.format == Format::Rgba16Float {
        let at =
            |k: usize| half::f16::from_le_bytes([texels[8 * i + 2 * k], texels[8 * i + 2 * k + 1]]).to_f32();
        let a = at(3).clamp(0.0, 1.0);
        let c = |k: usize| encode((at(k) / a.max(1e-6)).clamp(0.0, 1.0));
        return [c(0), c(1), c(2), a];
    }
    let b = |k: usize| f32::from(texels[4 * i + k]) / 255.0;
    let a = b(3);
    let c = |k: usize| (b(k) / a.max(1e-6)).clamp(0.0, 1.0);
    [c(0), c(1), c(2), a]
}

fn pack(c: [f32; 4]) -> f32 {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    f32::from_bits(byte(c[0]) | byte(c[1]) << 8 | byte(c[2]) << 16 | byte(c[3]) << 24)
}

pub(super) fn compact(texels: &[u8], shot: Asked, threshold: f32, out: &mut [[f32; 2]]) -> u32 {
    let (w, h) = (shot.size.0 as usize, shot.size.1 as usize);
    let n = w * h;
    let bytes = if shot.format == Format::Rgba16Float { 8 } else { 4 };
    if texels.len() < n * bytes {
        return 0;
    }
    let solid = |i: usize| texel(texels, shot, i)[3] > threshold;
    let total = (0..n).filter(|i| solid(*i)).count();
    let stride = total.div_ceil(MAX_POINTS).max(1);
    let mut count = 0;
    for i in (0..n).filter(|i| solid(*i)).step_by(stride).take(MAX_POINTS) {
        let q = shot.place.apply([(i % w) as f32 + 0.5, (i / w) as f32 + 0.5]);
        out[2 * count] = q;
        out[2 * count + 1] = [pack(texel(texels, shot, i)), 0.0];
        count += 1;
    }
    count as u32
}
