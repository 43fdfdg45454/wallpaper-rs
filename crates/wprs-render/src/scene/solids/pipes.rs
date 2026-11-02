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
    pub(crate) skinned: bool,
    pub(crate) shadow: bool,
    pub(crate) package: u64,
}

impl Key {
    fn code(self) -> (u16, u64) {
        let bits = u16::from(self.cull)
            | u16::from(self.blend) << 2
            | u16::from(self.write) << 4
            | u16::from(self.test) << 5
            | u16::from(self.hdr) << 6
            | u16::from(self.skinned) << 7
            | u16::from(self.shadow) << 8;
        (bits, if self.shadow { 0 } else { self.package })
    }
}

fn blend(key: Key) -> Option<wgpu::BlendState> {
    match key.blend {
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
    }
}

impl Solids {
    pub(crate) fn pipe(&self, device: &wgpu::Device, key: Key) -> Option<wgpu::RenderPipeline> {
        let mut pipes = self.pipes.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(p) = pipes.get(&key.code()) {
            return Some(p.clone());
        }
        let module = match key.code().1 {
            0 => (self.module(device), ""),
            k => (self.packages.lock().unwrap_or_else(PoisonError::into_inner).get(&k)?.clone()?, "wprs_"),
        };
        let made = self.build(device, key, (&module.0, module.1));
        pipes.insert(key.code(), made.clone());
        Some(made)
    }

    fn build(
        &self,
        device: &wgpu::Device,
        key: Key,
        (shader, pre): (&wgpu::ShaderModule, &str),
    ) -> wgpu::RenderPipeline {
        super::super::pipes::compiled(1);
        let base = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
        let paint = wgpu::vertex_attr_array![4 => Float32];
        let skin = wgpu::vertex_attr_array![5 => Float32x4, 6 => Float32x4];
        let step = wgpu::VertexStepMode::Vertex;
        let mut buffers = vec![
            Some(wgpu::VertexBufferLayout {
                array_stride: (wprs_scene::MODEL_FLOATS * 4) as u64,
                step_mode: step,
                attributes: &base,
            }),
            Some(wgpu::VertexBufferLayout { array_stride: 4, step_mode: step, attributes: &paint }),
        ];
        if key.skinned {
            buffers.push(Some(wgpu::VertexBufferLayout {
                array_stride: (wprs_scene::SKIN_FLOATS * 4) as u64,
                step_mode: step,
                attributes: &skin,
            }));
        }
        let layouts: Vec<Option<&wgpu::BindGroupLayout>> = self.layouts.iter().map(Some).collect();
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wprs-render solids"),
            bind_group_layouts: &layouts,
            immediate_size: 0,
        });
        let format = if key.hdr { Format::Rgba16Float } else { Format::Rgba8UnormSrgb };
        let cull = match key.cull {
            1 if !key.shadow => Some(wgpu::Face::Back),
            2 if !key.shadow => Some(wgpu::Face::Front),
            _ => None,
        };
        let compare = match (key.shadow, key.test) {
            (true, _) => wgpu::CompareFunction::LessEqual,
            (false, true) => wgpu::CompareFunction::GreaterEqual,
            (false, false) => wgpu::CompareFunction::Always,
        };
        let vs = match (key.shadow, key.skinned) {
            (true, false) => "vs_shadow",
            (true, true) => "vs_shadow_skinned",
            (false, false) => "vs_solid",
            (false, true) => "vs_skinned",
        };
        let vs = format!("{pre}{vs}");
        let fs = (!key.shadow).then(|| format!("{pre}fs_solid"));
        let targets = [Some(wgpu::ColorTargetState { blend: blend(key), ..format.into() })];
        let bias = match key.shadow {
            true => wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            false => Default::default(),
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wprs-render solids"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(&vs),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            fragment: fs.as_ref().map(|fs| wgpu::FragmentState {
                module: shader,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(key.write || key.shadow),
                depth_compare: Some(compare),
                stencil: Default::default(),
                bias,
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }
}
