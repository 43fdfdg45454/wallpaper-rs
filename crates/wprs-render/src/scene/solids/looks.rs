use super::pipes::Key;
use super::uniform::{Finish3, Object};
use wprs_scene::{Alpha, Color, Coverage, Cull, Finish, Look, Placed};

fn linear(c: [f32; 4]) -> [f32; 4] {
    let l = super::uniform::tone(Color(c), 1.0);
    [l[0], l[1], l[2], c[3]]
}

pub(super) fn object(p: &Placed) -> (Object, bool) {
    let m = &p.matrix;
    let a = |r: usize, c: usize| m[c * 4 + r];
    let cof = |r: usize, c: usize| {
        let (r1, r2, c1, c2) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        a(r1, c1) * a(r2, c2) - a(r1, c2) * a(r2, c1)
    };
    let det: f32 = (0..3).map(|c| a(0, c) * cof(0, c)).sum();
    let s = if det < 0.0 { -1.0 } else { 1.0 };
    let mut normal = [0.0; 16];
    (0..3).for_each(|c| (0..3).for_each(|r| normal[c * 4 + r] = cof(r, c) * s));
    normal[15] = s;
    (Object { model: *m, normal }, det < 0.0)
}

pub(super) fn finish(
    f: &Finish,
    look: Option<&Look>,
    opacity: f32,
    flipped: bool,
    hdr: bool,
) -> (Finish3, Key) {
    let mut color = linear(f.color);
    if let Some(c) = look.and_then(|l| l.color) {
        let c = linear(c);
        color = std::array::from_fn(|i| color[i] * c[i]);
    }
    color[3] *= opacity;
    let emissive = look.and_then(|l| l.emissive).map_or(f.emissive, |e| {
        let l = linear([e[0], e[1], e[2], 1.0]);
        [l[0], l[1], l[2]]
    });
    let n = look.map_or([None; 4], |l| l.numbers);
    let pbr = [
        n[0].unwrap_or(f.metallic),
        n[1].unwrap_or(f.roughness),
        n[2].unwrap_or(f.normal_scale),
        n[3].unwrap_or(f.occlusion),
    ];
    let unlit = f.unlit || look.is_some_and(|l| l.unlit);
    let lit = !unlit && look.and_then(|l| l.lit).unwrap_or(true);
    let cutoff = match f.alpha {
        Alpha::Mask(c) if look.and_then(|l| l.blend).is_none_or(|b| b == Coverage::Mask) => c.max(1e-4),
        _ => 0.0,
    };
    let mut blend = match (look.and_then(|l| l.blend), f.alpha) {
        (Some(Coverage::Blend), _) | (None, Alpha::Blend) => 1,
        (Some(Coverage::Add), _) => 2,
        _ => 0,
    };
    if blend == 0 && opacity < 1.0 {
        blend = 1;
    }
    let two = look.and_then(|l| l.double_sided).unwrap_or(f.double_sided);
    let cull = match look.and_then(|l| l.cull) {
        Some(Cull::None) => 0,
        Some(Cull::Front) => 2,
        Some(Cull::Back) => 1,
        None if two => 0,
        None => 1,
    };
    let cull = if flipped && cull > 0 { 3 - cull } else { cull };
    let test = look.and_then(|l| l.depth_test).unwrap_or(true);
    let write = look.and_then(|l| l.depth_write).unwrap_or(blend == 0);
    let uniform = Finish3 {
        color,
        emissive: [emissive[0], emissive[1], emissive[2], 0.0],
        pbr,
        flags: [cutoff, f32::from(u8::from(lit)), 0.0, 0.0],
        maps: [f32::from(u8::from(f.maps[2].is_some())), 0.0, 0.0, 0.0],
        more: [1.0, 0.0, 0.0, 0.0],
    };
    (uniform, Key { cull, blend, write, test, hdr })
}
