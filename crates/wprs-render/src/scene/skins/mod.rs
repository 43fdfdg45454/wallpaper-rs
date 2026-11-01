use wgpu::TextureFormat as Format;
use wprs_scene::VERTEX_FLOATS;

mod cast;
mod make;
#[cfg(test)]
mod sharp;
mod size;
#[cfg(test)]
mod steady;

pub(crate) use make::Skin;

pub(crate) const FORMAT: Format = Format::Rgba8UnormSrgb;
pub(crate) const DEPTH: Format = Format::Depth24PlusStencil8;
pub(crate) const BONES: usize = 768;
pub(crate) const PIECES: usize = 2 + 384;

pub(crate) struct Skinner {
    pub(crate) layout: wgpu::BindGroupLayout,
    pub(crate) pipes: [wgpu::RenderPipeline; 6],
}

fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
    let (vertex, fragment) = (wgpu::ShaderStages::VERTEX, wgpu::ShaderStages::FRAGMENT);
    let uniform = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let texture = |filterable| wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    let entries = [
        entry(0, vertex, uniform),
        entry(1, vertex, uniform),
        entry(2, vertex, texture(false)),
        entry(3, fragment, texture(true)),
        entry(4, fragment, sampler),
    ];
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("wprs-render skins"),
        entries: &entries,
    })
}

fn pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, k: usize) -> wgpu::RenderPipeline {
    let (clipped, deep, cull) = (k % 2 == 1, k >= 2, k >= 4);
    let shader = crate::shader::module(device, include_str!("skin.wgsl"));
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4, 4 => Float32];
    let buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: (VERTEX_FLOATS * 4) as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &attributes,
    })];
    let face = wgpu::StencilFaceState {
        compare: if clipped { wgpu::CompareFunction::Equal } else { wgpu::CompareFunction::Always },
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: if clipped { wgpu::StencilOperation::Keep } else { wgpu::StencilOperation::Replace },
    };
    let stencil = wgpu::StencilState { front: face, back: face, read_mask: 0xFF, write_mask: 0xFF };
    let depth_stencil = Some(wgpu::DepthStencilState {
        format: DEPTH,
        depth_write_enabled: Some(deep),
        depth_compare: Some(if deep {
            wgpu::CompareFunction::LessEqual
        } else {
            wgpu::CompareFunction::Always
        }),
        stencil,
        bias: Default::default(),
    });
    let pipelines = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("wprs-render skins"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    let blend = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("wprs-render skins"),
        layout: Some(&pipelines),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_skin"),
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_skin"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { blend, ..FORMAT.into() })],
        }),
        primitive: wgpu::PrimitiveState { cull_mode: cull.then_some(wgpu::Face::Back), ..Default::default() },
        depth_stencil,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(crate) fn skinner(device: &wgpu::Device) -> Skinner {
    let layout = layout(device);
    let pipes = std::array::from_fn(|k| pipeline(device, &layout, k));
    Skinner { layout, pipes }
}
