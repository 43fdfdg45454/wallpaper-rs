use super::super::item::{Item, Rect, scale_of};
use super::super::paint::{Painter, Target};
use super::super::{Op, Src};
use wprs_scene::{Affine, Content, Quality};

const MIRROR: f32 = 24.0;

impl Painter<'_, '_, '_> {
    pub(crate) fn lit(&self, i: usize) -> bool {
        let d = &self.frame.draws[i];
        d.lit.is_some()
            && matches!(d.content, Content::Image { .. } | Content::Video { .. } | Content::Text { .. })
    }

    pub(crate) fn baking(&self, i: usize) -> bool {
        self.frame.quality == Quality::Q0 && self.lit(i)
    }

    pub(crate) fn shifting(&self, range: std::ops::Range<usize>) -> bool {
        let glow = &self.pool.glow;
        (glow.moved || glow.live) && self.frame.draws[range].iter().any(|d| d.lit.is_some())
    }

    fn map_of(&mut self, id: Option<&str>) -> Option<Src> {
        let images = self.images;
        let found = id.and_then(|id| self.moving(id).or_else(|| images.get(id).map(|i| &i.0)))?;
        Some(self.ext(found))
    }

    pub(crate) fn light(
        &mut self,
        i: usize,
        entry: &'static str,
        item: &mut Item,
        (m, bounds): (Affine, Rect),
        target: Target,
    ) -> Option<(&'static str, [Src; 8], Src)> {
        let entry = match entry {
            "fs_picture" => "fs_lit_picture",
            "fs_texture" => "fs_lit_texture",
            "fs_text" => "fs_lit_text",
            _ => return None,
        };
        let l = self.frame.draws[i].lit.filter(|_| self.lit(i))?;
        item.lit = [l.strength, l.roughness, l.reflection, l.emissive];
        let [a, b, c, d, ..] = m.0;
        let (x, y) = (a.hypot(b).max(1e-6), c.hypot(d).max(1e-6));
        item.lit_basis = [a / x, b / x, c / y, d / y];
        item.lit_at = [target.rect.x, target.rect.y, 1.0, MIRROR * scale_of(self.screen)];
        let found = [l.normal, l.roughness_map, l.emissive_map].map(|id| self.map_of(id));
        let mut maps = [Src::White; 8];
        maps[4..].copy_from_slice(&self.gobos());
        for (k, src) in found.into_iter().enumerate() {
            item.lit_maps[k] = f32::from(u8::from(src.is_some()));
            maps[k] = src.unwrap_or(Src::White);
        }
        let mut behind = Src::Clear;
        if l.reflection > 0.0 {
            match self.map_of(l.env) {
                Some(env) => (maps[3], item.lit_maps[3]) = (env, 1.0),
                None => {
                    let (x, y) = (bounds.x - target.rect.x, bounds.y - target.rect.y);
                    if let Some(back) = self.acquire(bounds.bucket().size()) {
                        let at = [0, 0];
                        let (origin, size) = ([x as u32, y as u32], bounds.size());
                        self.ops.push(Op::Copy { from: target.src, to: back, origin, size, at });
                        let (w, h) = self.size_of(back);
                        item.lit_env = [-x / w as f32, -y / h as f32, 1.0 / w as f32, 1.0 / h as f32];
                        (maps[3], item.lit_maps[3], behind) = (back, 2.0, back);
                    }
                }
            }
        }
        Some((entry, maps, behind))
    }
}
