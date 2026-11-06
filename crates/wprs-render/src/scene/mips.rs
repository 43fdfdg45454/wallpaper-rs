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

const COPY: &str = "
@group(0) @binding(0) var src: texture_2d<f32>;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Out(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0), uv);
}
@fragment fn fs(in: Out) -> @location(0) vec4<f32> { return textureLoad(src, vec2<i32>(in.position.xy), 0); }
";

pub(crate) fn level(n: u32) -> wgpu::TextureViewDescriptor<'static> {
    wgpu::TextureViewDescriptor { base_mip_level: n, mip_level_count: Some(1), ..Default::default() }
}

impl Pipes {
    pub(crate) fn shrinker(&self, device: &Device, format: TextureFormat) -> wgpu::RenderPipeline {
        self.blitter(device, format, false)
    }

    fn blitter(&self, device: &Device, format: TextureFormat, copy: bool) -> wgpu::RenderPipeline {
        let mut shrinks = self.shrinks.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let made = shrinks.entry((format, copy)).or_insert_with(|| {
            super::pipes::compiled(1);
            let module = super::pipes::module(device, if copy { COPY } else { SHRINK }.into());
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
        self.blit(device, encoder, (from, to), (format, false));
    }

    pub(crate) fn blit(
        &self,
        device: &Device,
        encoder: &mut CommandEncoder,
        (from, to): (&TextureView, &TextureView),
        (format, copy): (TextureFormat, bool),
    ) {
        let pipeline = self.blitter(device, format, copy);
        let source = wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(from) };
        let sampler =
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) };
        let entries = if copy { vec![source] } else { vec![source, sampler] };
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
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

impl crate::Renderer {
    pub(crate) fn exact(&self, image: &crate::GpuImage) -> Option<crate::GpuImage> {
        if image.crop == [1.0; 2] {
            return None;
        }
        let (w, h) = image.size();
        let size = ((w as f32 * image.crop[0]).round() as u32, (h as f32 * image.crop[1]).round() as u32);
        let levels = 32 - size.0.max(size.1).max(1).leading_zeros();
        let format = crate::packed::drawable(image.texture.format());
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wprs-render exact picture"),
            size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let pipes = self.pipes();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let first = texture.create_view(&level(0));
        pipes.blit(&self.device, &mut encoder, (&image.view, &first), (format, true));
        for n in 1..levels {
            let (from, to) = (texture.create_view(&level(n - 1)), texture.create_view(&level(n)));
            pipes.shrink(&self.device, &mut encoder, &from, &to, format);
        }
        self.queue.submit([encoder.finish()]);
        Some(crate::texture::gpu_image(texture))
    }
}
