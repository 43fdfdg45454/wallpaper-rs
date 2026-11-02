use super::pipes::Key;
use super::uniform::DrawSlots;
use super::{BONES_SLOT, FINISH_SLOT, SLOT, Staged};
use wprs_scene::{Frame, Stage3d};

#[derive(Clone, Copy)]
pub(crate) struct Call {
    pub(super) model: u64,
    pub(super) first: u32,
    pub(super) count: u32,
    pub(super) maps: usize,
    pub(super) object: u32,
    pub(super) finish: u32,
    pub(super) bones: u32,
    pub(super) draw: u32,
    pub(super) shells: u32,
    pub(super) cast: bool,
    pub(super) key: Key,
    pub(super) far: f32,
}

pub(super) fn sized(size: [f32; 2], scale: f32, frame: &Frame, edge: u32) -> (u32, u32) {
    let k = scale * frame.quality.render_scale();
    let [w, h] = size.map(|v| ((v * k).round() as u32).clamp(1, edge));
    (w, h)
}

pub(super) fn put<T: bytemuck::Pod>(bytes: &mut Vec<u8>, at: u64, value: &T) {
    put_bytes(bytes, at, bytemuck::bytes_of(value));
}

pub(super) fn put_bytes(bytes: &mut Vec<u8>, at: u64, raw: &[u8]) {
    let at = at as usize;
    if bytes.len() < at + raw.len() {
        bytes.resize(at + raw.len(), 0);
    }
    bytes[at..at + raw.len()].copy_from_slice(raw);
}

pub(super) fn rooms(frame: &Frame, stage: &Stage3d) -> (usize, usize, usize) {
    let placed = &frame.placed[stage.models.clone()];
    let finishes: usize = placed.iter().map(|p| p.model.materials.len() + 1).sum();
    let draws: usize =
        placed.iter().filter(|p| !p.model.morphs.is_empty()).map(|p| p.model.primitives.len()).sum();
    let bones = placed.iter().filter(|p| p.model.posed()).count();
    (finishes, draws + 1, bones)
}

pub(super) fn plan(s: &mut Staged, frame: &Frame, stage: &Stage3d) {
    let (mut calls, mut objects, mut finishes) = (
        std::mem::take(&mut s.calls),
        std::mem::take(&mut s.objects_bytes),
        std::mem::take(&mut s.finish_bytes),
    );
    let (mut bones, mut draws) = (std::mem::take(&mut s.bone_bytes), std::mem::take(&mut s.draw_bytes));
    calls.clear();
    objects.clear();
    finishes.clear();
    bones.clear();
    draws.clear();
    put(&mut draws, 0, &DrawSlots::default());
    let (mut slot, mut posed, mut drawn) = (0u32, 0u32, 1u32);
    for (o, p) in frame.placed[stage.models.clone()].iter().enumerate() {
        let (object, flipped) = super::looks::object(p);
        put(&mut objects, o as u64 * SLOT, &object);
        let skinned = p.model.posed();
        let bone = if skinned { posed } else { 0 };
        if skinned {
            let palette = p.pose.map_or(&p.model.rest[..], |q| &q.palette[..]);
            let flat: &[f32] = bytemuck::cast_slice(palette);
            put_bytes(
                &mut bones,
                u64::from(posed) * BONES_SLOT,
                bytemuck::cast_slice(&flat[..flat.len().min(768 * 4)]),
            );
            posed += 1;
        }
        let looks = &frame.looks[p.looks.clone()];
        let far = stage.eye.depth([p.matrix[12], p.matrix[13], p.matrix[14]]);
        let defaults = wprs_scene::Finish::default();
        for (m, f) in p.model.materials.iter().chain([&defaults]).enumerate() {
            let look = looks.iter().find(|l| usize::from(l.material) == m);
            let shape = (p.opacity, flipped, p.model.radius());
            let (uniform, key, shells) = super::looks::finish(f, look, shape, (s.hdr, skinned));
            put(&mut finishes, u64::from(slot + m as u32) * FINISH_SLOT, &uniform);
            let cast = look.and_then(|l| l.cast).unwrap_or(true) && key.blend == 0;
            let mine =
                |q: &&wprs_scene::Primitive| q.material.map_or(p.model.materials.len(), usize::from) == m;
            for prim in p.model.primitives.iter().filter(mine) {
                let weights = p.pose.and_then(|q| q.morphs.get(usize::from(prim.piece)));
                let draw = match weights.filter(|w| w.iter().any(|(_, x)| *x != 0.0)) {
                    Some(w) => {
                        let mut d =
                            DrawSlots { first: [prim.base as f32, 1.0, 0.0, 0.0], ..Default::default() };
                        for (k, (b, x)) in w.iter().enumerate() {
                            d.slots[k / 2][(k % 2) * 2] = *b as f32;
                            d.slots[k / 2][(k % 2) * 2 + 1] = *x;
                        }
                        put(&mut draws, u64::from(drawn) * SLOT, &d);
                        drawn += 1;
                        drawn - 1
                    }
                    None => 0,
                };
                let (first, count, maps, object, finish) =
                    (prim.first, prim.count, m, o as u32, slot + m as u32);
                calls.push(Call {
                    model: p.model.digest,
                    first,
                    count,
                    maps,
                    object,
                    finish,
                    bones: bone,
                    draw,
                    shells,
                    cast,
                    key,
                    far,
                });
            }
        }
        slot += p.model.materials.len() as u32 + 1;
    }
    calls.sort_by(|a, b| {
        (a.key.blend > 0).cmp(&(b.key.blend > 0)).then(match a.key.blend > 0 {
            true => b.far.total_cmp(&a.far),
            false => std::cmp::Ordering::Equal,
        })
    });
    (s.calls, s.objects_bytes, s.finish_bytes, s.bone_bytes, s.draw_bytes) =
        (calls, objects, finishes, bones, draws);
}
