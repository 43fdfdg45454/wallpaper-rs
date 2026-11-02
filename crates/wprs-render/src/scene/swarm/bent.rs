use crate::scene::item::{Item, ndc};
use crate::scene::paint::{Painter, Target};
use crate::scene::pipes::Mode;
use crate::scene::{Op, Src};
use wgpu::TextureFormat;

impl Painter<'_, '_, '_> {
    pub(crate) fn lit_swarms(&self) -> bool {
        self.swarms.as_ref().is_some_and(|s| s.kits.iter().any(|k| k.look.light[0] > 0.5))
    }

    pub(super) fn bend(&mut self, target: Target, flock: usize, slot: u32) {
        let Some(scale) = self.frame.quality.bend() else { return };
        if self.state_format != TextureFormat::Rgba16Float {
            return;
        }
        let (w, h) = target.rect.size();
        let small = (((w as f32) * scale).ceil() as u32, ((h as f32) * scale).ceil() as u32);
        let Some(shift) = self.borrow(small, TextureFormat::Rgba16Float).map(Src::Pool) else { return };
        let Some(back) = self.acquire((w, h)) else { return self.release(shift) };
        self.ops.push(Op::Clear(shift, [0.0; 4]));
        let (system, mode) = (flock, Mode::Normal);
        self.ops.push(Op::Particles { target: shift, system, slot, mode, timed: 0, bend: true });
        self.ops.push(Op::Copy { from: target.src, to: back, origin: [0, 0], size: (w, h), at: [0, 0] });
        let full = [0.0, 0.0, 1.0, 1.0];
        let mut item = Item { quad: full, uv: full, color: [1.0; 4], ..Item::default() };
        (item.m0, item.m1) = ndc(target.rect, target.rect.affine());
        item.lit_env = [0.0, 0.0, 1.0 / w as f32, 1.0 / h as f32];
        let tex = [back, shift, Src::Clear];
        let slot = self.pool.push(&item);
        self.ops.push(Op::Draw { target: target.src, entry: "fs_refract", mode: Mode::Replace, slot, tex });
        self.release(back);
        self.release(shift);
    }
}
