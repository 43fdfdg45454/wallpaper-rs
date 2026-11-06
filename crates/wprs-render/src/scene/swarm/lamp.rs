use super::kit::Kit;
use crate::Renderer;
use crate::scene::Image;
use std::collections::HashMap;
use wprs_scene::Render;

pub(crate) struct Lamp<'a> {
    pub(crate) layout: &'a wgpu::BindGroupLayout,
    pub(crate) lights: &'a wgpu::Buffer,
    pub(crate) images: &'a HashMap<String, Image>,
    pub(crate) white: &'a wgpu::TextureView,
    pub(crate) layers: bool,
}

fn flag(b: bool) -> f32 {
    f32::from(u8::from(b))
}

pub(super) fn light(r: &Renderer, kit: &mut Kit, (render, sprite): (&Render, [f32; 2]), lamp: &Lamp<'_>) {
    let [sx, sy] = sprite;
    kit.look.crops = [1.0 / sx, 1.0 / sy, 1.0 / sx, 1.0 / sy];
    let bend = render.refract.as_ref();
    if !render.lit && render.normal.is_none() && bend.is_none() && !lamp.layers {
        return;
    }
    let exact = |id: Option<&String>| {
        let found = id.and_then(|id| lamp.images.get(id)).map(|i| &i.0);
        found.and_then(|i| r.exact(i)).or_else(|| found.cloned())
    };
    let (normal, bent) = (exact(render.normal.as_ref()), exact(bend.map(|b| &b.normal)));
    let (normal, bent) =
        (normal.as_ref().map_or(lamp.white, |i| &i.view), bent.as_ref().map_or(lamp.white, |i| &i.view));
    let entries = [
        wgpu::BindGroupEntry { binding: 0, resource: lamp.lights.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(normal) },
        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(bent) },
    ];
    let desc = wgpu::BindGroupDescriptor { label: None, layout: lamp.layout, entries: &entries };
    kit.lit = Some(r.device.create_bind_group(&desc));
    let normal = render.normal.as_ref().is_some_and(|id| lamp.images.contains_key(id));
    kit.look.light = [flag(render.lit), flag(normal), 0.0, 1.0];
    kit.bend = bend.map(|b| b.amount.clamp(0.0, 1.0));
    kit.look.bend = [kit.bend.unwrap_or(0.0), 0.0, 0.0, 0.0];
}
