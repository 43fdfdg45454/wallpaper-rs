use super::mode;
use crate::Renderer;
use crate::scene::item::{linear, ndc};
use crate::scene::paint::{Painter, Target};
use crate::scene::{Image, Op};
use bytemuck::{Pod, Zeroable};
use wprs_scene::{Affine, Blend, Content, Frames, Mask, Render, Scene, Stack};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct Look {
    pub(super) m0: [f32; 4],
    pub(super) m1: [f32; 4],
    pub(super) tint: [f32; 4],
    pub(super) atlas: [f32; 4],
    pub(super) trail: [f32; 4],
    pub(super) shade: [f32; 4],
    pub(super) extra: [f32; 4],
    pub(super) sizes: [[f32; 4]; 16],
    pub(super) colors: [[f32; 4]; 64],
}

pub(super) struct Kit {
    pub(super) id: String,
    pub(super) system: usize,
    pub(super) layer: Option<String>,
    pub(super) look: Look,
    pub(super) order: Option<(Stack, f32)>,
    pub(super) trail: u32,
    pub(super) tex: wgpu::BindGroup,
}

type Sprite<'a> = (Option<(&'a str, Option<&'a Image>)>, &'a [[f32; 4]]);

#[allow(clippy::too_many_arguments)]
pub(super) fn kit(
    r: &Renderer,
    layout: &wgpu::BindGroupLayout,
    (system, layer): (usize, Option<String>),
    render: &Render,
    scene: &Scene,
    (sprite, cells): Sprite,
    white: &wgpu::TextureView,
    srgb: bool,
) -> Kit {
    let (sizes, colors) = render.luts();
    let atlas = match cells.len() {
        0 => sprite.and_then(|(t, _)| scene.assets.get(t)?.atlas).unwrap_or([1, 1]),
        n => [1, n.min(256) as u32],
    };
    let image = sprite.and_then(|(_, image)| image);
    let frames = match render.frames {
        None => 0.0,
        Some(Frames::Time) => 1.0,
        Some(Frames::Life) => 2.0,
        Some(Frames::Random) => 3.0,
    };
    let tone = |c: [f32; 4]| if srgb { c } else { [linear(c[0]), linear(c[1]), linear(c[2]), c[3]] };
    let flag = |b: bool| f32::from(u8::from(b));
    let stretch = render.stretch;
    let look = Look {
        tint: [1.0, flag(srgb), 1.0, flag(image.is_some())],
        atlas: [atlas[1] as f32, atlas[0] as f32, frames, render.fps],
        trail: [stretch.time, 0.0, 0.0, flag(render.mix)],
        shade: [
            1.0,
            render.cycles.unwrap_or(1) as f32,
            stretch.min.unwrap_or(0.0),
            stretch.max.unwrap_or(1e9),
        ],
        extra: [flag(!cells.is_empty()), 0.0, 0.0, render.trail.as_ref().map_or(1.0, |t| t.width)],
        sizes: std::array::from_fn(|i| std::array::from_fn(|j| sizes[i * 4 + j])),
        colors: colors.map(tone),
        ..Zeroable::zeroed()
    };
    let view = image.map_or(white, |i| &i.0.view);
    let mut table = [[0.0f32; 4]; 256];
    table.iter_mut().zip(cells).for_each(|(t, c)| *t = *c);
    let rects = crate::filled::filled(
        (&r.device, &r.queue),
        "wprs-scene particle frames",
        bytemuck::cast_slice(&table),
        wgpu::BufferUsages::UNIFORM,
    );
    let entries = [
        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
        wgpu::BindGroupEntry { binding: 1, resource: rects.as_entire_binding() },
    ];
    let tex =
        r.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries });
    let id = scene.particles.keys().nth(system).cloned().unwrap_or_default();
    let largest = scene.particles.values().nth(system).map_or(1.0, |s| wprs_scene::largest(s, render));
    let (order, trail) = (render.order.map(|o| (o, largest)), render.trail.as_ref().map_or(0, |t| t.length));
    Kit { id, system, layer, look, order, trail, tex }
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
        let Content::Particles { flock, size, .. } = d.content else { return };
        if mask.is_some() || !blend.fixed() {
            return self.apart(i, target, atop);
        }
        let (r, screen, srgb) = (self.r, self.screen, self.srgb);
        let Some(swarms) = self.swarms.as_deref_mut() else { return };
        let mode = mode(blend, atop);
        swarms.pipeline(r, mode);
        let Some(herd) = swarms.herds.get(flock) else { return };
        let [w, h] = swarms.canvas;
        let m = match herd.world {
            true => screen,
            false => screen.then(d.world).then(Affine::translate([-d.origin[0] * w, -d.origin[1] * h])),
        };
        let tinted = d.tint != wprs_scene::Color([1.0; 4]) || d.brightness != 1.0;
        let [cr, cg, cb, _] = d.tint.0;
        let toned = crate::scene::item::toned(wprs_scene::Color([cr, cg, cb, 1.0]), d.brightness, srgb, 1.0);
        let tint = [toned[0], toned[1], toned[2], d.tint.0[3]];
        let slot = (swarms.looks.len() as u64 * super::LOOK) as u32;
        for (p, system) in herd.bake.systems.iter().enumerate() {
            let own = (p == 0).then_some(d.layer.id.as_str());
            let found = |layer: Option<&str>| {
                swarms.kits.iter().position(|k| k.system == *system && k.layer.as_deref() == layer)
            };
            let Some(k) = own.and_then(|l| found(Some(l))).or_else(|| found(None)) else { continue };
            let kit = &swarms.kits[k];
            let mut look = kit.look;
            if tinted {
                look.colors.iter_mut().for_each(|c| (0..4).for_each(|j| c[j] *= tint[j]));
            }
            (look.m0, look.m1) = ndc(target.rect, m);
            look.tint[0] = opacity;
            look.tint[2] = herd.bake.sizes[p] * size;
            let trail = herd.bake.trails[p];
            look.extra[1] = swarms.quality.trail(kit.trail.min(trail)) as f32;
            look.extra[2] = trail as f32;
            swarms.looks.push(look);
            swarms.kitted.push(k);
        }
        self.ops.push(Op::Particles { target: target.src, system: flock, slot, mode });
    }
}
