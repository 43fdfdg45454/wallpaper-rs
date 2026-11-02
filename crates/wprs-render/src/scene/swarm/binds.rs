use super::kit::Look;
use crate::Renderer;
use wgpu::{BindGroupLayout, Buffer, BufferUsages as B, ShaderStages as S};

pub(super) fn entry(binding: u32, visibility: S, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
}

pub(super) fn storage(read_only: bool) -> wgpu::BindingType {
    let ty = wgpu::BufferBindingType::Storage { read_only };
    wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }
}

pub(super) fn uniform(size: usize) -> wgpu::BindingType {
    let (ty, min_binding_size) = (wgpu::BufferBindingType::Uniform, wgpu::BufferSize::new(size as u64));
    wgpu::BindingType::Buffer { ty, has_dynamic_offset: true, min_binding_size }
}

pub(super) fn plain(size: u64) -> wgpu::BindingType {
    let (ty, min_binding_size) = (wgpu::BufferBindingType::Uniform, wgpu::BufferSize::new(size));
    wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size }
}

pub(super) fn buffer(r: &Renderer, size: u64, usage: B) -> Buffer {
    let label = Some("wprs-scene particles");
    r.device.create_buffer(&wgpu::BufferDescriptor { label, size, usage, mapped_at_creation: false })
}

pub(super) fn module(r: &Renderer, wgsl: &str) -> wgpu::ShaderModule {
    let source = wgpu::ShaderSource::Wgsl(wgsl.into());
    r.device
        .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("wprs-scene particles"), source })
}

pub(super) fn layouts(r: &Renderer) -> [BindGroupLayout; 7] {
    let layout = |entries: &[wgpu::BindGroupLayoutEntry]| {
        let label = Some("wprs-scene particles");
        r.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label, entries })
    };
    let filtering = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    let sample_type = wgpu::TextureSampleType::Float { filterable: true };
    let (view_dimension, c) = (wgpu::TextureViewDimension::D2, S::COMPUTE);
    let texture = wgpu::BindingType::Texture { sample_type, view_dimension, multisampled: false };
    let rects = plain(4096);
    let sim = uniform(size_of::<wprs_scene::Sim>());
    [
        layout(&[
            entry(0, c, storage(false)),
            entry(1, c, storage(false)),
            entry(2, c, storage(true)),
            entry(3, c, storage(true)),
            entry(4, c, sim),
        ]),
        layout(&[entry(0, S::VERTEX_FRAGMENT, uniform(size_of::<Look>())), entry(1, S::FRAGMENT, filtering)]),
        layout(&[entry(0, S::FRAGMENT, texture), entry(1, S::FRAGMENT, rects)]),
        layout(&[
            entry(0, c, storage(true)),
            entry(1, c, storage(false)),
            entry(2, c, storage(false)),
            entry(3, c, plain(16)),
        ]),
        layout(&[entry(0, c, storage(true)), entry(1, c, storage(false)), entry(2, c, sim)]),
        layout(&[
            entry(0, c, storage(true)),
            entry(1, c, storage(true)),
            entry(2, c, storage(false)),
            entry(3, c, sim),
        ]),
        layout(&[
            entry(0, S::FRAGMENT, plain(size_of::<crate::scene::lamps::Lights>() as u64)),
            entry(1, S::FRAGMENT, texture),
            entry(2, S::FRAGMENT, texture),
        ]),
    ]
}
