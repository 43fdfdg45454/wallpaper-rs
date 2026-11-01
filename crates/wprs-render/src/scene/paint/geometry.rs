use super::super::item::{Rect, scale_of, shape};
use super::super::sampling::Sampling;
use super::Painter;
use wprs_scene::{Affine, Content};

impl<'a> Painter<'a, '_, '_> {
    pub(crate) fn geometry(&self, i: usize) -> Option<([f32; 2], [f32; 4])> {
        let frame = self.frame;
        let d = &frame.draws[i];
        let unit = [0.0, 0.0, 1.0, 1.0];
        match &d.content {
            Content::Image { size, .. } | Content::Video { size, .. } => {
                Some((size.or(self.natural(i))?, unit))
            }
            Content::Text { .. } => {
                let (size, [rx, ry]) = self.lettered(i)?;
                Some((size, [-rx, -ry, 1.0 + rx, 1.0 + ry]))
            }
            Content::Solid { size, .. } => Some((*size, unit)),
            Content::Spectrum { spectrum, .. } => Some((spectrum.size, unit)),
            Content::Particles { .. } => Some((self.swarms.as_ref()?.canvas, unit)),
            Content::Shape { shape: s, stroke, .. } => {
                let (size, ..) = shape(s);
                let px = 2.0 / scale_of(self.screen.then(d.world)).max(1e-6);
                let m = stroke.map_or(0.0, |s| s.1 / 2.0) + px;
                Some((size, [-m / size[0], -m / size[1], 1.0 + m / size[0], 1.0 + m / size[1]]))
            }
            _ => None,
        }
    }

    pub(crate) fn lettered(&self, i: usize) -> Option<([f32; 2], [f32; 2])> {
        let Content::Text { layout, .. } = &self.frame.draws[i].content else { return None };
        let raster = self.text(i)?.1;
        let size = raster.map(|v| (v - 2.0 * layout.reach).max(1.0));
        Some((size, [0, 1].map(|k| (raster[k] - size[k]) / 2.0 / size[k])))
    }

    pub(crate) fn cell(&self, i: usize) -> ([f32; 4], Option<&'a Sampling>) {
        let frame = self.frame;
        let (asset, cell) = match &frame.draws[i].content {
            Content::Image { asset, frame, .. } => (asset, *frame),
            Content::Video { asset, .. } => (asset, None),
            _ => return ([0.0, 0.0, 1.0, 1.0], None),
        };
        let found = self.samplings.get(asset.as_ref());
        let rect = cell.and_then(|c| found?.cells.get(c as usize).copied());
        (rect.unwrap_or([0.0, 0.0, 1.0, 1.0]), found)
    }

    pub(crate) fn natural(&self, i: usize) -> Option<[f32; 2]> {
        let frame = self.frame;
        let (Content::Image { asset, .. } | Content::Video { asset, .. }) = &frame.draws[i].content else {
            return None;
        };
        let natural = self.moving(asset).map(|v| [v.size().0 as f32, v.size().1 as f32]);
        let [w, h] = natural.or_else(|| self.images.get(asset.as_ref()).map(|image| image.1))?;
        let rect = self.cell(i).0;
        Some([w * rect[2], h * rect[3]])
    }

    pub(crate) fn local(&self, i: usize, size: [f32; 2]) -> Affine {
        let frame = self.frame;
        let d = &frame.draws[i];
        if let Content::Particles { flock, .. } = d.content
            && self.swarms.as_ref().is_some_and(|s| s.world(flock))
        {
            return self.screen.then(frame.camera).then(Affine::scale(size));
        }
        let shift = Affine::translate([-d.origin[0] * size[0], -d.origin[1] * size[1]]);
        self.screen.then(d.world).then(shift).then(Affine::scale(size))
    }

    fn reach(&self, i: usize) -> f32 {
        let frame = self.frame;
        let d = &frame.draws[i];
        self.frame.effects[d.effects.clone()].iter().map(|e| e.reach).sum::<f32>() * scale_of(self.screen)
    }

    pub(crate) fn extent(&self, i: usize) -> Option<Rect> {
        if self.lean(i).is_none() || matches!(self.frame.draws[i].content, Content::Adjustment) {
            return self.level(i);
        }
        let [x0, y0, x1, y1] = self.within(i, |p| p.level(i)).map(|r| [r.x, r.y, r.x + r.w, r.y + r.h])?;
        self.map(i, Affine::IDENTITY).bounds([x0, y0, x1, y1])
    }

    fn level(&self, i: usize) -> Option<Rect> {
        let frame = self.frame;
        let d = &frame.draws[i];
        let rect = match &d.content {
            Content::Group { .. } => {
                let mut j = i + 1;
                let mut found: Option<Rect> = None;
                while j < d.end {
                    if let Some(r) = self.extent(j).filter(|_| !self.frame.draws[j].hidden) {
                        found = Some(found.map_or(r, |f| f.union(r)));
                    }
                    j = self.frame.draws[j].end;
                }
                found?
            }
            Content::Adjustment => self.full,
            _ => {
                let (size, [x0, y0, x1, y1]) = self.geometry(i)?;
                let m = self.local(i, size);
                Rect::around([[x0, y0], [x1, y0], [x0, y1], [x1, y1]].map(|p| m.apply(p)))?
            }
        };
        Some(rect.grow(self.reach(i)))
    }
}
