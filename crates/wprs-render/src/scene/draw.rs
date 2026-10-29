use super::item::{Item, Rect, channel, gradient, invert, ndc, premultiplied, rows, scale_of, shape, toned};
use super::paint::{Painter, Target};
use super::pipes::Mode;
use super::{Op, Src};
use wprs_scene::{Affine, Blend, Content, Layout, Mask, SpectrumStyle, Wrapping};

impl Painter<'_, '_, '_> {
    fn content(&mut self, i: usize, opacity: f32, size: [f32; 2]) -> Option<(&'static str, Item, Src)> {
        let frame = self.frame;
        let d = &frame.draws[i];
        let srgb = self.srgb;
        let mut item = Item { uv: [0.0, 0.0, 1.0, 1.0], color: [opacity; 4], ..Item::default() };
        item.size = [0.0, 0.0, 1.0, f32::from(u8::from(srgb))];
        match &d.content {
            Content::Image { asset, .. } | Content::Video { asset, .. } => {
                let (images, videos) = (self.images, self.videos);
                let video = videos.iter().find(|(id, _)| id == asset).map(|(_, v)| *v);
                let found = video.or_else(|| images.get(asset.as_ref()).map(|(i, _)| i))?;
                let (rect, s) = self.cell(i);
                let natural = self.natural(i).unwrap_or(size);
                let wrap = s.filter(|_| video.is_none()).map_or(Wrapping::Clamp, |s| s.wrap);
                let tiles = match wrap {
                    Wrapping::Clamp => [1.0; 2],
                    _ => [0, 1].map(|k| size[k] / natural[k].max(1e-3)),
                };
                let nearest = s.is_some_and(|s| s.nearest) && video.is_none();
                (item.uv, item.color) = (rect, toned(d.tint, d.brightness, srgb, opacity));
                item.shape = [1.0 + wrap as u8 as f32, f32::from(u8::from(nearest)), tiles[0], tiles[1]];
                Some(("fs_picture", item, self.ext(found)))
            }
            Content::Text { color, .. } => {
                let texts = self.texts;
                item.color = premultiplied(*color, srgb, opacity);
                Some(("fs_texture", item, self.ext(texts.get(&i)?.0)))
            }
            Content::Solid { color, .. } => {
                item.color = premultiplied(*color, srgb, opacity);
                Some(("fs_solid", item, Src::White))
            }
            Content::Shape { shape: s, fill, gradient: g, stroke } => {
                let (size, params, place) = shape(s);
                (item.size, item.shape, item.place) =
                    ([size[0], size[1], 0.0, 0.0], params, [place[0], place[1], 0.0, 0.0]);
                item.color = fill.map_or([0.0; 4], |c| premultiplied(c, srgb, opacity));
                item.style[1] = f32::from(u8::from(fill.is_some() || g.is_some()));
                if let Some(g) = g {
                    gradient(&mut item, g, srgb);
                    item.color = [opacity; 4];
                }
                if let Some((color, width)) = stroke {
                    (item.stroke, item.style[0]) = (premultiplied(*color, srgb, opacity), *width);
                }
                Some(("fs_shape", item, Src::White))
            }
            Content::Spectrum { spectrum, color } => {
                item.color = premultiplied(*color, srgb, opacity);
                item.size = [spectrum.size[0], spectrum.size[1], 0.0, 0.0];
                let style = match spectrum.style {
                    SpectrumStyle::Bars => 0.0,
                    SpectrumStyle::Circle => 1.0,
                    SpectrumStyle::Wave => 2.0,
                };
                item.mode = [0.0, style, spectrum.bands as f32, spectrum.gap];
                Some(("fs_spectrum", item, Src::White))
            }
            _ => None,
        }
    }

    pub(crate) fn direct(
        &mut self,
        i: usize,
        target: Target,
        (opacity, blend, mask): (f32, Blend, Option<&Mask>),
        atop: bool,
    ) {
        if matches!(self.frame.draws[i].content, Content::Particles { .. }) {
            return self.particles(i, target, (opacity, blend, mask), atop);
        }
        let Some((size, quad)) = self.geometry(i) else { return };
        let Some((entry, mut item, src)) = self.content(i, opacity, size) else { return };
        let b = self.frame.draws[i].brightness;
        if !matches!(self.frame.draws[i].content, Content::Image { .. } | Content::Video { .. }) {
            [&mut item.color, &mut item.stroke]
                .into_iter()
                .for_each(|c| c[..3].iter_mut().for_each(|v| *v *= b));
            item.stop_colors.iter_mut().for_each(|c| c[..3].iter_mut().for_each(|v| *v *= b));
        }
        let m = self.local(i, size);
        let [x0, y0, x1, y1] = quad;
        let Some(bounds) = Rect::around([[x0, y0], [x1, y0], [x0, y1], [x1, y1]].map(|p| m.apply(p)))
            .and_then(|r| r.pixels(target.rect))
        else {
            return;
        };
        item.quad = quad;
        (item.m0, item.m1) = ndc(target.rect, m);
        let masked = self.mask(&mut item, (i, mask), m, size, target);
        let (mode, back) = self.blend(&mut item, blend, bounds, target, atop);
        if let Some(mut backing) = self.backing(i, m, size, opacity, target) {
            (backing.mask_info, backing.channel, backing.mask_m0, backing.mask_m1, backing.mask_shape) =
                (item.mask_info, item.channel, item.mask_m0, item.mask_m1, item.mask_shape);
            let slot = self.pool.push(&backing);
            let tex = [Src::White, masked, back];
            self.ops.push(Op::Draw { target: target.src, entry: "fs_shape", mode, slot, tex });
        }
        let slot = self.pool.push(&item);
        self.ops.push(Op::Draw { target: target.src, entry, mode, slot, tex: [src, masked, back] });
    }

    fn backing(&self, i: usize, m: Affine, size: [f32; 2], opacity: f32, target: Target) -> Option<Item> {
        let d = &self.frame.draws[i];
        let Content::Text { layout: Layout { backing: Some((color, _, radius)), padding, .. }, .. } =
            &d.content
        else {
            return None;
        };
        let inner = [0, 1].map(|k| (size[k] - 2.0 * padding).max(1.0));
        let box_m = m
            .then(Affine::translate([padding / size[0], padding / size[1]]))
            .then(Affine::scale([inner[0] / size[0], inner[1] / size[1]]));
        let px = 2.0 / scale_of(self.screen.then(d.world)).max(1e-6);
        let [ex, ey] = [px / inner[0], px / inner[1]];
        let mut item = Item {
            quad: [-ex, -ey, 1.0 + ex, 1.0 + ey],
            color: toned(*color, d.brightness, self.srgb, opacity),
            size: [inner[0], inner[1], 0.0, 0.0],
            shape: [0.0, radius.min(inner[0].min(inner[1]) / 2.0), 0.0, 0.0],
            place: [inner[0] / 2.0, inner[1] / 2.0, 0.0, 0.0],
            style: [0.0, 1.0, 0.0, 0.0],
            ..Item::default()
        };
        (item.m0, item.m1) = ndc(target.rect, box_m);
        Some(item)
    }

    pub(crate) fn composite(&mut self, i: usize, src: Src, rect: Rect, target: Target, atop: bool) {
        let frame = self.frame;
        let d = &frame.draws[i];
        let Some(bounds) = rect.pixels(target.rect) else { return };
        let group = matches!(d.content, Content::Group { .. });
        let color = if group { toned(d.tint, d.brightness, self.srgb, d.opacity) } else { [d.opacity; 4] };
        let mut item =
            Item { quad: [0.0, 0.0, 1.0, 1.0], uv: [0.0, 0.0, 1.0, 1.0], color, ..Item::default() };
        (item.m0, item.m1) = ndc(target.rect, rect.affine());
        let (m, size) = self
            .geometry(i)
            .map_or((rect.affine(), [rect.w, rect.h]), |(size, _)| (self.local(i, size), size));
        let masked = self.mask(&mut item, (i, d.mask), m, size, target);
        let (mode, back) = self.blend(&mut item, d.blend, bounds, target, atop);
        let slot = self.pool.push(&item);
        self.ops.push(Op::Draw {
            target: target.src,
            entry: "fs_texture",
            mode,
            slot,
            tex: [src, masked, back],
        });
    }

    pub(crate) fn adjust(&mut self, i: usize, target: Target) {
        let frame = self.frame;
        let d = &frame.draws[i];
        if d.effects.is_empty() {
            return;
        }
        let size = target.rect.size();
        let Some(copy) = self.acquire(size) else { return };
        self.ops.push(Op::Copy { from: target.src, to: copy, origin: [0, 0], size, at: [0, 0] });
        let out = self.effects(i, copy, target.rect, invert(self.stage).then(target.rect.affine()));
        let mut item = Item {
            quad: [0.0, 0.0, 1.0, 1.0],
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [d.opacity; 4],
            ..Item::default()
        };
        (item.m0, item.m1) = ndc(target.rect, target.rect.affine());
        let mode = if d.opacity >= 1.0 { Mode::Replace } else { Mode::Normal };
        let slot = self.pool.push(&item);
        self.ops.push(Op::Draw {
            target: target.src,
            entry: "fs_texture",
            mode,
            slot,
            tex: [out, Src::White, Src::Clear],
        });
    }

    fn mask(
        &mut self,
        item: &mut Item,
        (i, mask): (usize, Option<&Mask>),
        m: Affine,
        size: [f32; 2],
        target: Target,
    ) -> Src {
        let Some(mask) = mask else { return Src::White };
        let frame = self.frame;
        let texture = frame.draws[i].mask_texture.as_deref();
        let to_local = invert(m).then(Affine::translate([target.rect.x, target.rect.y]));
        let weak = 1.0 - frame.draws[i].mask_strength;
        item.mask_info = [1.0, f32::from(u8::from(mask.invert)), mask.feather, weak];
        item.channel = channel(mask.channel, mask.texture.is_some());
        let images = self.images;
        let (affine, src) = if let Some(id) = texture {
            let image = images.get(id).map(|(image, _)| image);
            (to_local, image.map_or(Src::White, |image| self.ext(image)))
        } else if let Some(id) = &mask.layer {
            item.mask_info[0] = 2.0;
            let found = self.frame.draws.iter().position(|d| &d.layer.id == id).and_then(|j| self.isolate(j));
            let (src, r) = found.unwrap_or((Src::Clear, target.rect));
            let back = Affine([
                1.0 / r.w,
                0.0,
                0.0,
                1.0 / r.h,
                (target.rect.x - r.x) / r.w,
                (target.rect.y - r.y) / r.h,
            ]);
            (back, src)
        } else if let Some(s) = &mask.shape {
            let (shape_size, params, _) = shape(s);
            item.mask_info[0] = 3.0;
            item.mask_shape = [params[0], params[1], shape_size[0], shape_size[1]];
            let center =
                Affine::translate([(shape_size[0] - size[0]) / 2.0, (shape_size[1] - size[1]) / 2.0]);
            (center.then(Affine::scale(size)).then(to_local), Src::White)
        } else {
            return Src::White;
        };
        (item.mask_m0, item.mask_m1) = rows(affine);
        src
    }

    fn blend(
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
