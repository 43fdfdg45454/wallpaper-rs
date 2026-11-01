use super::{PARTICLE, Swarms};
use crate::Renderer;
use crate::scene::pipes::{Mode, blend};

impl Swarms {
    pub(super) fn pipeline(&mut self, r: &Renderer, mode: Mode) {
        if self.pipelines.contains_key(&mode) {
            return;
        }
        let sprites = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4];
        let near = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float16x2, 2 => Unorm8x4];
        let far = wgpu::vertex_attr_array![3 => Float32x2, 4 => Float16x2, 5 => Unorm8x4];
        let layouts = [&self.layouts[1], &self.layouts[2]];
        let module = &self.module;
        let sprite =
            self.build(r, mode, (module, ("vs_particle", "fs_particle")), &layouts, &[(PARTICLE, &sprites)]);
        let ribbon =
            self.build(r, mode, (module, ("vs_trail", "fs_trail")), &layouts, &[(16, &near), (16, &far)]);
        self.pipelines.insert(mode, sprite);
        self.ribbons.insert(mode, ribbon);
        self.created += 2;
        self.staged(r, mode);
    }

    pub(super) fn build(
        &self,
        r: &Renderer,
        mode: Mode,
        (module, (vs, fs)): (&wgpu::ShaderModule, (&str, &str)),
        groups: &[&wgpu::BindGroupLayout],
        slots: &[(u64, &[wgpu::VertexAttribute])],
    ) -> wgpu::RenderPipeline {
        let layouts: Vec<_> = groups.iter().map(|g| Some(*g)).collect();
        let desc =
            wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &layouts, immediate_size: 0 };
        let layout = r.device.create_pipeline_layout(&desc);
        let step_mode = wgpu::VertexStepMode::Instance;
        let buffers: Vec<_> = slots
            .iter()
            .map(|&(array_stride, attributes)| {
                Some(wgpu::VertexBufferLayout { array_stride, step_mode, attributes })
            })
            .collect();
        let target = wgpu::ColorTargetState { blend: blend(mode), ..self.format.into() };
        let compilation_options = wgpu::PipelineCompilationOptions::default();
        r.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wprs-scene particles"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vs),
                compilation_options: compilation_options.clone(),
                buffers: &buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options,
                targets: &[Some(target)],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }
}
