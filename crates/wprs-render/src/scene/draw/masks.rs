use super::super::item::{Item, Rect, channel, shape};
use super::super::map::Map;
use super::super::paint::{Painter, Target};
use super::super::pipes::Mode;
use super::super::{Op, Src};
use wprs_scene::{Affine, Blend, Mask};

impl Painter<'_, '_, '_> {
    pub(crate) fn mask(
        &mut self,
        item: &mut Item,
        (mask, texture, strength): (Option<&Mask>, Option<&str>, f32),
        m: Map,
        size: [f32; 2],
        target: Target,
    ) -> Src {
        let Some(mask) = mask else { return Src::White };
        let to_local = m.inverse().then(Affine::translate([target.rect.x, target.rect.y]));
        let weak = 1.0 - strength;
        item.mask_info = [1.0, f32::from(u8::from(mask.invert)), mask.feather, weak];
        item.channel = channel(mask.channel, mask.texture.is_some());
        let (affine, src) = if let Some(id) = texture {
            (to_local, self.picture(id).map_or(Src::White, |image| self.ext(image)))
        } else if let Some(id) = &mask.layer {
            item.mask_info[0] = 2.0;
            let found = self.frame.draws.iter().position(|d| &d.layer.id == id).and_then(|j| self.shared(j));
            let (src, r) = found.unwrap_or((Src::Clear, target.rect));
            let back = Affine([
                1.0 / r.w,
                0.0,
                0.0,
                1.0 / r.h,
                (target.rect.x - r.x) / r.w,
                (target.rect.y - r.y) / r.h,
            ]);
            (Map::Flat(back), src)
        } else if let Some(s) = &mask.shape {
            let (shape_size, params, _) = shape(s);
            item.mask_info[0] = 3.0;
            item.mask_shape = [params[0], params[1], shape_size[0], shape_size[1]];
            let center =
                Affine::translate([(shape_size[0] - size[0]) / 2.0, (shape_size[1] - size[1]) / 2.0]);
            (to_local.after(center.then(Affine::scale(size))), Src::White)
        } else {
            return Src::White;
        };
        affine.sample(item);
        src
    }

    pub(crate) fn blend(
        &mut self,
        item: &mut Item,
        blend: Blend,
        bounds: Rect,
        target: Target,
        atop: bool,
    ) -> (Mode, Src) {
        let fixed = match blend {
            Blend::Normal if atop => Some(Mode::Atop),
            Blend::Normal => Some(Mode::Normal),
            Blend::Add => Some(Mode::Add),
            Blend::Multiply => Some(Mode::Multiply),
            Blend::Screen => Some(Mode::Screen),
            _ => None,
        };
        if let Some(mode) = fixed {
            return (mode, Src::Clear);
        }
        let (x, y) = (bounds.x - target.rect.x, bounds.y - target.rect.y);
        let size = bounds.size();
        let Some(back) = self.acquire(bounds.bucket().size()) else { return (Mode::Normal, Src::Clear) };
        self.ops.push(Op::Copy {
            from: target.src,
            to: back,
            origin: [x as u32, y as u32],
            size,
            at: [0, 0],
        });
        let (w, h) = self.size_of(back);
        let (w, h) = (w as f32, h as f32);
        item.back = [-x / w, -y / h, 1.0 / w, 1.0 / h];
        item.mode[0] = blend as u32 as f32;
        (Mode::Replace, back)
    }
}
