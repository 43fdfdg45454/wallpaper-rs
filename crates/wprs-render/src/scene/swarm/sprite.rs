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
        if self.kits.iter().any(|k| k.rope.is_some() || k.long) {
            self.corded(r, mode);
        }
        self.staged(r, mode);
    }

    fn corded(&mut self, r: &Renderer, mode: Mode) {
        let layouts = [&self.layouts[1], &self.layouts[2]];
        let module = &self.module;
        let (a, b) = (
            wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
            wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4],
        );
        let rope = self.build(r, mode, (module, ("vs_rope", "fs_cord")), &layouts, &[(48, &a), (48, &b)]);
        let (p, q) = (
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float16x2],
            wgpu::vertex_attr_array![9 => Float32x2, 10 => Float16x2],
        );
        let (ta, tb) = (
            wgpu::vertex_attr_array![3 => Float32x2, 4 => Float16x2, 5 => Unorm8x4],
            wgpu::vertex_attr_array![6 => Float32x2, 7 => Float16x2, 8 => Unorm8x4],
        );
        let slots = [(16, &p[..]), (16, &ta[..]), (16, &tb[..]), (16, &q[..])];
        let long = self.build(r, mode, (module, ("vs_long", "fs_cord")), &layouts, &slots);
        self.cords.insert(mode, [rope, long]);
        self.created += 2;
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
