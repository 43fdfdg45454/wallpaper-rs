use wgpu::ShaderStages as S;

fn entry(binding: u32, visibility: S, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
}

fn uniform(dynamic: bool) -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: dynamic,
        min_binding_size: None,
    }
}

fn picture() -> wgpu::BindingType {
    wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    }
}

pub(super) fn layouts(device: &wgpu::Device) -> [wgpu::BindGroupLayout; 4] {
    let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    let both = S::VERTEX | S::FRAGMENT;
    let make = |label: &str, entries: &[wgpu::BindGroupLayoutEntry]| {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries })
    };
    let maps: Vec<_> =
        (0..5).map(|k| entry(k, S::FRAGMENT, picture())).chain([entry(5, S::FRAGMENT, sampler)]).collect();
    [
        make(
            "wprs-render solid scene",
            &[
                entry(0, both, uniform(true)),
                entry(1, S::FRAGMENT, picture()),
                entry(2, S::FRAGMENT, sampler),
            ],
        ),
        make("wprs-render solid object", &[entry(0, S::VERTEX, uniform(true))]),
        make("wprs-render solid maps", &maps),
        make("wprs-render solid finish", &[entry(0, S::FRAGMENT, uniform(true))]),
    ]
}
