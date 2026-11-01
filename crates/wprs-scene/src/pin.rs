use crate::figure::{Attach, Follows};
use crate::puppet::limb::flat;
use crate::puppet::mount::{mount_on, size};
use crate::resolve::Context;
use crate::{Affine, Kind, Lean, MAX_DEPTH};

pub(crate) fn parts(f: Affine) -> ([f32; 2], f32, [f32; 2]) {
    let [a, b, c, d, e, g] = f.0;
    let sx = a.hypot(b).max(1e-6);
    ([e, g], b.atan2(a).to_degrees(), [sx, (a * d - b * c) / sx])
}

impl Context<'_> {
    pub(crate) fn pinned(&self, a: &Attach, depth: usize) -> Option<(Affine, Option<Lean>, bool)> {
        let target = self.scene.layer(&a.layer).filter(|_| depth < 2 * MAX_DEPTH)?;
        let place = self.placed(target, None, depth + 1);
        let found = self.puppets.iter().find(|p| p.layer == target.id);
        let name = a.bone.as_deref().or(a.point.as_deref())?;
        let frame = match (&target.kind, found) {
            (Kind::Mesh(m), Some(p)) => {
                let w = p.worlds.get(p.rig.joint(name)?)?;
                let at = mount_on(self, target, size(self, target, m), place.world);
                at.then(flat(w)).then(Affine::scale([1.0, -1.0]))
            }
            _ => place.world,
        };
        let has = |f: Follows| a.inherit.as_ref().is_none_or(|l| l.contains(&f));
        let (at, turn, scale) = parts(frame);
        let at = if has(Follows::Position) { at } else { [place.world.0[4], place.world.0[5]] };
        let base = Affine::translate(at)
            .then(Affine::rotate(if has(Follows::Rotation) { turn } else { 0.0 }))
            .then(Affine::scale(if has(Follows::Scale) { scale } else { [1.0; 2] }))
            .then(Affine::translate(a.offset.unwrap_or_default()));
        Some((base, place.lean, place.visible))
    }
}
