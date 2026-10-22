use crate::color::LUT;
use crate::decode::{scaled, shown};
use crate::parallax::{DepthMap, halves};
use crate::shader::{bind_group_layout, module, pipeline};
use crate::stack::{Plan, plan};
use crate::texture::{GpuImage, create_texture, gpu_image, texture};
use crate::{RenderError, Renderer};
use image::RgbaImage;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindingResource as Resource, CommandEncoder as Encoder, TextureFormat as Format};
use wgpu::{TextureUsages as Usage, TextureView};
use wprs_core::{Edit, UvRect};

pub(crate) struct Developer {
    pub(crate) develop: wgpu::RenderPipeline,
    output: wgpu::RenderPipeline,
    blur: [wgpu::RenderPipeline; 2],
    pub(crate) layout: wgpu::BindGroupLayout,
    pub(crate) format: Format,
}

impl Developer {
    pub(crate) fn new(r: &Renderer) -> Self {
        let offered = r.gpu.get_texture_format_features(Format::Rgba16Float);
        let half = offered.allowed_usages.contains(Usage::RENDER_ATTACHMENT)
            && offered.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);
        let format = if half { Format::Rgba16Float } else { Format::Rgba8UnormSrgb };
        let layout = bind_group_layout(&r.device, 5);
        let (develop, kawase) =
            (module(&r.device, include_str!("develop.wgsl")), module(&r.device, include_str!("kawase.wgsl")));
        let make =
            |layout, shader, entry, format: Format| pipeline(&r.device, layout, shader, entry, format.into());
        Self {
            develop: make(&layout, &develop, "fs_develop", format),
            output: make(&layout, &develop, "fs_output", Format::Rgba8UnormSrgb),
            blur: ["fs_down", "fs_up"].map(|entry| make(&r.layout, &kawase, entry, format)),
            layout,
            format,
        }
    }
}

fn levels(radius: f32, (w, h): (u32, u32)) -> u32 {
    ((radius / 2.0).log2().ceil().max(1.0) as u32).min(6).min(w.min(h).max(1).ilog2().max(1))
}

fn view(texture: &wgpu::Texture) -> TextureView {
    texture.create_view(&Default::default())
}

impl Renderer {
    pub fn develop_size(&self, frame: (u32, u32), uv: UvRect, target: (u32, u32)) -> (u32, u32) {
        let [fw, fh] = [frame.0, frame.1].map(|v| v.max(1) as f32);
        let factor = shown(frame, uv, target).min(1.0).min(self.max_edge() as f32 / fw.max(fh));
        ((fw * factor).round().max(1.0) as u32, (fh * factor).round().max(1.0) as u32)
    }

    pub(crate) fn developer(&self) -> &Developer {
        self.developer.get_or_init(|| Developer::new(self))
    }

    pub fn develop(
        &self,
        image: &RgbaImage,
        edit: &Edit,
        depth: Option<&DepthMap>,
        size: (u32, u32),
    ) -> Result<GpuImage, RenderError> {
        self.check_target(size.0, size.1)?;
        let frame = edit.geometry.frame(image.dimensions());
        let factor = (size.0 as f32 / frame.0 as f32).max(size.1 as f32 / frame.1 as f32);
        let (source, mut plan) =
            (scaled(image, factor, self.max_edge()), plan(edit, image.dimensions(), size, depth.is_some()));
        self.scoped(|| {
            let d = self.developer();
            let [src, lut, aux, dep] = self.inputs(&source, &plan, depth);
            let mut encoder = self.device.create_command_encoder(&Default::default());
            let ends: Vec<u32> = plan.blurs.iter().map(|(i, _)| *i).chain([plan.stack.end]).collect();
            let pair: Vec<_> =
                (0..ends.len().min(2)).map(|_| self.target(size, d.format, Usage::empty())).collect();
            let deepest = plan.blurs.iter().map(|(_, radius)| levels(*radius, size) + 1).max().unwrap_or(0);
            let chain: Vec<_> = (0..deepest)
                .map(|i| self.target((size.0 >> i, size.1 >> i), d.format, Usage::empty()))
                .collect();
            let (mut current, mut blurred, mut start) = (src, None, 0);
            for (k, end) in ends.into_iter().enumerate() {
                (plan.stack.start, plan.stack.end) = (start, end);
                let out = &pair[k % 2];
                let views = [&current, blurred.as_ref().unwrap_or(&current), &lut, &aux, &dep];
                self.pass(&mut encoder, &d.develop, &d.layout, bytemuck::bytes_of(&plan.stack), &views, out);
                blurred =
                    plan.blurs.get(k).map(|(_, radius)| self.kawase(&mut encoder, d, out, &chain, *radius));
                (current, start) = (view(out), end);
            }
            let out = self.target(size, Format::Rgba8UnormSrgb, Usage::COPY_SRC);
            let views = [&current, &current, &lut, &aux, &dep];
            self.pass(&mut encoder, &d.output, &d.layout, bytemuck::bytes_of(&plan.stack), &views, &out);
            self.queue.submit([encoder.finish()]);
            gpu_image(out)
        })
    }

    fn inputs(&self, source: &RgbaImage, plan: &Plan, depth: Option<&DepthMap>) -> [TextureView; 4] {
        let image =
            |i: &RgbaImage, format| view(&texture(self, i.as_raw(), i.dimensions(), i.width() * 4, format));
        let src = image(source, Format::Rgba8UnormSrgb);
        let ((curves, rows), blank) = (&plan.lut, vec![0u16; 4 * LUT]);
        let curves = bytemuck::cast_slice(if *rows == 0 { &blank } else { curves });
        let lut = texture(self, curves, (LUT as u32, (*rows).max(1)), 8 * LUT as u32, Format::Rgba16Float);
        let aux = plan.brush.as_ref().map_or_else(|| src.clone(), |b| image(b, Format::Rgba8Unorm));
        let dep = depth.map_or_else(
            || src.clone(),
            |m| {
                view(&texture(
                    self,
                    bytemuck::cast_slice(&halves(m)),
                    m.dimensions(),
                    m.width() * 2,
                    Format::R16Float,
                ))
            },
        );
        [src, view(&lut), aux, dep]
    }

    pub(crate) fn target(&self, (w, h): (u32, u32), format: Format, extra: Usage) -> wgpu::Texture {
        let usage = Usage::RENDER_ATTACHMENT | Usage::TEXTURE_BINDING | extra;
        create_texture(&self.device, "wprs-render develop", (w.max(1), h.max(1)), format, usage)
    }

    fn kawase(
        &self,
        encoder: &mut Encoder,
        d: &Developer,
        source: &wgpu::Texture,
        chain: &[wgpu::Texture],
        radius: f32,
    ) -> TextureView {
        let levels = levels(radius, (source.width(), source.height())) as usize;
        let offset = radius / (1u32 << levels) as f32;
        let mut blur = |pipeline, from: &wgpu::Texture, to: &wgpu::Texture| {
            let params = [1.0 / from.width() as f32, 1.0 / from.height() as f32, offset, 0.0];
            self.pass(encoder, pipeline, &self.layout, bytemuck::bytes_of(&params), &[&view(from)], to)
        };
        for i in 0..levels {
            blur(&d.blur[0], if i == 0 { source } else { &chain[i] }, &chain[i + 1]);
        }
        for i in (0..levels).rev() {
            blur(&d.blur[1], &chain[i + 1], &chain[i]);
        }
        view(&chain[0])
    }

    pub(crate) fn pass(
        &self,
        encoder: &mut Encoder,
        pipeline: &wgpu::RenderPipeline,
        layout: &wgpu::BindGroupLayout,
        uniform: &[u8],
        views: &[&TextureView],
        target: &wgpu::Texture,
    ) {
        let usage = wgpu::BufferUsages::UNIFORM;
        let label = Some("wprs-render develop params");
        let buffer =
            self.device.create_buffer_init(&BufferInitDescriptor { label, contents: uniform, usage });
        let textures = views.iter().map(|v| Resource::TextureView(v));
        let resources = std::iter::once(buffer.as_entire_binding())
            .chain(textures)
            .chain([Resource::Sampler(&self.clamp)]);
        let entries: Vec<_> = resources
            .enumerate()
            .map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource })
            .collect();
        let group =
            self.device.create_bind_group(&wgpu::BindGroupDescriptor { label, layout, entries: &entries });
        let ops =
            wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store };
        let view = view(target);
        let attachment =
            wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render develop pass"),
            color_attachments: &[Some(attachment)],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
