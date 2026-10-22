use super::mode;
use crate::scene::item::{linear, ndc};
use crate::scene::paint::{Painter, Target};
use crate::scene::{Image, Op};
use bytemuck::{Pod, Zeroable};
use wprs_scene::{Affine, Bake, Blend, Content, Frames, Mask, STEP, Scene, System};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct Look {
    pub(super) m0: [f32; 4],
    pub(super) m1: [f32; 4],
    pub(super) tint: [f32; 4],
    pub(super) atlas: [f32; 4],
    pub(super) trail: [f32; 4],
    pub(super) sizes: [[f32; 4]; 16],
    pub(super) colors: [[f32; 4]; 64],
}

pub(super) struct Kit {
    pub(super) id: String,
    pub(super) bake: Bake,
    pub(super) look: Look,
    pub(super) trail: u32,
    pub(super) texture: wgpu::TextureView,
}

pub(super) fn kit(
    id: &str,
    s: &System,
    bake: Bake,
    scene: &Scene,
    image: Option<&Image>,
    white: &wgpu::TextureView,
    srgb: bool,
) -> Kit {
    let (sizes, colors) = s.render.luts();
    let r = &s.render;
    let atlas = r.texture.as_ref().and_then(|t| scene.assets.get(t)?.atlas).unwrap_or([1, 1]);
    let frames = match r.frames {
        None => 0.0,
        Some(Frames::Time) => 1.0,
        Some(Frames::Life) => 2.0,
        Some(Frames::Random) => 3.0,
    };
    let tone = |c: [f32; 4]| if srgb { c } else { [linear(c[0]), linear(c[1]), linear(c[2]), c[3]] };
    let flag = |b: bool| f32::from(u8::from(b));
    let look = Look {
        tint: [1.0, flag(srgb), bake.size, flag(image.is_some())],
        atlas: [atlas[1] as f32, atlas[0] as f32, frames, r.fps],
        trail: [r.stretch, 0.0, r.trail.as_ref().map_or(1.0, |t| t.width), flag(r.mix)],
        sizes: std::array::from_fn(|i| std::array::from_fn(|j| sizes[i * 4 + j])),
        colors: colors.map(tone),
        ..Zeroable::zeroed()
    };
    let trail = r.trail.as_ref().map_or(0, |t| t.length.min(bake.trail));
    let texture = image.map_or_else(|| white.clone(), |(i, _)| i.view.clone());
    Kit { id: id.into(), bake, look, trail, texture }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn particles(
        &mut self,
        i: usize,
        target: Target,
        (opacity, blend, mask): (f32, Blend, Option<&Mask>),
        atop: bool,
    ) {
        let frame = self.frame;
        let d = &frame.draws[i];
        let Content::Particles { system } = d.content else { return };
        if mask.is_some() || !blend.fixed() {
            if let Some((src, rect)) = self.isolate(i) {
                self.composite(i, src, rect, target, atop);
            }
            return;
        }
        let (r, screen) = (self.r, self.screen);
        let Some(swarms) = self.swarms.as_deref_mut() else { return };
        let Some(index) = swarms.kits.iter().position(|k| k.id == system) else { return };
        let mode = mode(blend, atop);
        swarms.pipeline(r, mode);
        let [w, h] = swarms.canvas;
        let kit = &swarms.kits[index];
        let mut look = kit.look;
        let m = screen.then(d.world).then(Affine::translate([-d.origin[0] * w, -d.origin[1] * h]));
        (look.m0, look.m1) = ndc(target.rect, m);
        (look.tint[0], look.trail[1]) = (opacity, swarms.quality.trail(kit.trail) as f32 * STEP);
        let slot = (swarms.looks.len() as u64 * super::LOOK) as u32;
        swarms.looks.push(look);
        self.ops.push(Op::Particles { target: target.src, system: index, slot, mode });
    }
}
