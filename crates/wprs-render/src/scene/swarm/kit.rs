use crate::Renderer;
use crate::scene::Image;
use crate::scene::item::linear;
use bytemuck::{Pod, Zeroable};
use wprs_scene::{Frames, Render, Scene, Stack};

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
