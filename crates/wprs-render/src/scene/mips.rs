use super::pipes::Pipes;
use wgpu::{CommandEncoder, Device, TextureFormat, TextureView};

pub(crate) const SHRINK: &str = "
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Out(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0), uv);
}
@fragment fn fs(in: Out) -> @location(0) vec4<f32> { return textureSampleLevel(src, samp, in.uv, 0.0); }
";

pub(crate) fn level(n: u32) -> wgpu::TextureViewDescriptor<'static> {
    wgpu::TextureViewDescriptor { base_mip_level: n, mip_level_count: Some(1), ..Default::default() }
}

impl Pipes {
    pub(crate) fn shrinker(&self, device: &Device, format: TextureFormat) -> wgpu::RenderPipeline {
        let mut shrinks = self.shrinks.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let made = shrinks.entry(format).or_insert_with(|| {
            super::pipes::compiled(1);
            let module = super::pipes::module(device, SHRINK.into());
            let state = |entry| (Some(entry), Default::default());
            let ((vs, vo), (fs, fo)) = (state("vs"), state("fs"));
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("wprs-scene mipmaps"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: vs,
                    compilation_options: vo,
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: fs,
                    compilation_options: fo,
                    targets: &[Some(format.into())],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        made.clone()
    }

    pub(crate) fn shrink(
        &self,
        device: &Device,
        encoder: &mut CommandEncoder,
        from: &TextureView,
        to: &TextureView,
        format: TextureFormat,
    ) {
        let pipeline = self.shrinker(device, format);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(from) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let ops = wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            store: wgpu::StoreOp::Store,
        };
        let attachment =
            wgpu::RenderPassColorAttachment { view: to, depth_slice: None, resolve_target: None, ops };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-scene mipmaps"),
            color_attachments: &[Some(attachment)],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
