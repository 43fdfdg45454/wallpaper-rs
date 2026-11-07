use super::Src;
use super::item::{rows, toned};
use super::paint::Painter;
use wprs_scene::{Affine, Content, EffectDraw, MaskMode, Output, PassDef};

const PLAIN: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

pub(crate) type View = [[f32; 4]; 3];
pub(crate) type Virtual = (Src, (u32, u32), View);

fn reads_once(list: &[EffectDraw<'_>]) -> bool {
    let n = list.first().map_or(0, |e| e.chain.clamp(1, list.len()));
    let clean = list[..n].iter().all(|e| !e.descriptor.wgsl.contains("input_tex") && !e.descriptor.below());
    let named = |p: &PassDef, n: &[&str]| p.inputs.iter().chain(&p.copy).any(|i| n.contains(&i.as_str()));
    let source = list.iter().flat_map(|e| &e.descriptor.passes).any(|p| named(p, &["input", "source"]));
    let Some(e) = list.first() else { return false };
    let d = e.descriptor;
    let [p] = d.passes.as_slice() else { return false };
    let again = p.inputs.iter().skip(1).any(|i| i == "previous");
    let bare = p.target.is_none() && p.copy.is_none() && p.blend.is_none() && p.vertex.is_none();
    let mixed = d.mask_mode == MaskMode::Mix && e.mask.is_some();
    let first = p.inputs.first().is_some_and(|i| i == "previous") && p.entry != "fs_copy";
    let once = d.state.is_none() && d.targets.is_empty() && p.output == Output::Next;
    clean && !source && bare && first && !again && !mixed && once
}

impl Painter<'_, '_, '_> {
    pub(crate) fn skinned(&mut self, j: usize, m: Affine) -> Option<(Src, [f32; 4])> {
        let d = &self.frame.draws[j];
        let Content::Mesh { layer, .. } = &d.content else { return None };
        let (size, quad) = self.geometry(j)?;
        let alone = d.end == j + 1 && d.effects.is_empty() && d.mask.is_none() && d.lean.is_none();
        let plain =
            alone && self.r.tiled() && !self.srgb && self.flat.get().is_none() && self.backdrop.is_none();
        let white = toned(d.tint, d.brightness, false, 1.0) == [1.0; 4];
        let near = |a: Affine| a.0.iter().zip(m.0).all(|(x, y)| (x - y).abs() <= 1e-3 * (1.0 + y.abs()));
        let fits = plain && white && quad == [0.0, 0.0, 1.0, 1.0] && near(self.local(j, size));
        let skin = self.skins.get(*layer).filter(|_| fits)?.image.clone();
        Some((self.ext(&skin), [0.0; 4]))
    }

    pub(crate) fn pictured(&mut self, i: usize, uv: Affine) -> Option<(Src, View)> {
        let frame = self.frame;
        let d = &frame.draws[i];
        let shown = matches!(d.content, Content::Image { .. } | Content::Video { .. } | Content::Mesh { .. });
        let alone = d.end == i + 1 && d.mask.is_none() && d.mask_texture.is_none() && d.lit.is_none();
        let plain =
            shown && alone && d.lean.is_none() && self.flat.get().is_none() && self.backdrop.is_none();
        let list = &frame.effects[d.effects.clone()];
        let ready = list.first().is_some_and(|e| !self.cut(i, e)) && reads_once(list);
        if !(plain && ready && self.r.tiled() && !self.srgb) {
            return None;
        }
        let (size, _) = self.geometry(i).filter(|(_, q)| *q == [0.0, 0.0, 1.0, 1.0])?;
        let (entry, item, src) = self.content(i, 1.0, size)?;
        let Src::Ext(k) = src else { return None };
        let flat =
            item.shape == PLAIN && item.size[3] == 0.0 && item.mode[1] == 0.0 && item.color == [1.0; 4];
        if entry != "fs_picture" || !flat || self.exts[k].texture.mip_level_count() != 1 {
            return None;
        }
        let (m0, m1) = rows(uv);
        let mode = if item.size[2] > 0.5 { 1.0 } else { 2.0 };
        Some((src, [[m0[0], m0[1], m0[2], mode], [m1[0], m1[1], m1[2], 0.0], item.uv]))
    }
}
