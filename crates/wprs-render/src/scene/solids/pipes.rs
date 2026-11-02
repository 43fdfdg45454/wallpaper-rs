use super::{DEPTH, Solids};
use std::sync::PoisonError;
use wgpu::TextureFormat as Format;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    pub(crate) cull: u8,
    pub(crate) blend: u8,
    pub(crate) write: bool,
    pub(crate) test: bool,
    pub(crate) hdr: bool,
}

impl Key {
    fn code(self) -> u16 {
        u16::from(self.cull)
            | u16::from(self.blend) << 2
            | u16::from(self.write) << 4
            | u16::from(self.test) << 5
            | u16::from(self.hdr) << 6
    }
}

impl Solids {
    pub(crate) fn pipe(&self, device: &wgpu::Device, key: Key) -> wgpu::RenderPipeline {
        let mut pipes = self.pipes.lock().unwrap_or_else(PoisonError::into_inner);
        pipes.entry(key.code()).or_insert_with(|| self.build(device, key)).clone()
    }

    fn build(&self, device: &wgpu::Device, key: Key) -> wgpu::RenderPipeline {
        super::super::pipes::compiled(1);
        let shader = crate::shader::module(device, include_str!("solid.wgsl"));
        let attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: (wprs_scene::MODEL_FLOATS * 4) as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let layouts: Vec<Option<&wgpu::BindGroupLayout>> = self.layouts.iter().map(Some).collect();
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wprs-render solids"),
            bind_group_layouts: &layouts,
            immediate_size: 0,
        });
        let blend = match key.blend {
            0 => None,
            1 => Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            _ => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }),
        };
        let format = if key.hdr { Format::Rgba16Float } else { Format::Rgba8UnormSrgb };
        let cull = match key.cull {
            1 => Some(wgpu::Face::Back),
            2 => Some(wgpu::Face::Front),
            _ => None,
        };
        let compare =
            if key.test { wgpu::CompareFunction::GreaterEqual } else { wgpu::CompareFunction::Always };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wprs-render solids"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_solid"),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_solid"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { blend, ..format.into() })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(key.write),
                depth_compare: Some(compare),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }
}
