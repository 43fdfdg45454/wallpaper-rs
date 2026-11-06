use crate::Renderer;
use crate::shader::{bind_group_layout, module, pipeline};
use crate::texture::create_texture;
use wgpu::TextureFormat as Format;
use wgpu::TextureUsages as Usage;

pub(crate) struct Pq {
    key: ((u32, u32), Format),
    view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

pub fn pq_code(nits: f32) -> f32 {
    let y = (nits / 10_000.0).clamp(0.0, 1.0).powf(0.159_301_76);
    ((0.835_937_5 + 18.851_563 * y) / (1.0 + 18.6875 * y)).powf(78.843_75)
}

impl Pq {
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub(crate) fn finish(
        &self,
        r: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        light: [f32; 2],
    ) {
        r.queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&[light[0], light[1], 0.0, 0.0]));
        let ops =
            wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store };
        let attachment =
            wgpu::RenderPassColorAttachment { view: out, depth_slice: None, resolve_target: None, ops };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render pq pass"),
            color_attachments: &[Some(attachment)],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl Renderer {
    pub(crate) fn pq<'a>(&self, slot: &'a mut Option<Box<Pq>>, size: (u32, u32), out: Format) -> &'a Pq {
        if slot.as_ref().is_none_or(|p| p.key != (size, out)) {
            *slot = Some(Box::new(self.pq_slot(size, out)));
        }
        slot.as_ref().expect("made above")
    }

    fn pq_slot(&self, size: (u32, u32), out: Format) -> Pq {
        let usage = Usage::TEXTURE_BINDING | Usage::RENDER_ATTACHMENT;
        let texture = create_texture(&self.device, "wprs-render pq light", size, Format::Rgba16Float, usage);
        let view = texture.create_view(&Default::default());
        let layout = bind_group_layout(&self.device, 1);
        let shader = module(&self.device, include_str!("pq.wgsl"));
        let pipeline = pipeline(&self.device, &layout, &shader, "fs_main", out.into());
        let usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let uniform = crate::filled::filled((&self.device, &self.queue), "wprs-render pq", &[0; 16], usage);
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render pq"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.clamp) },
            ],
        });
        Pq { key: (size, out), view, uniform, group, pipeline }
    }
}

#[cfg(test)]
mod tests {
    use super::pq_code;

    #[test]
    fn pq_codes_follow_st_2084() {
        assert!(pq_code(0.0) < 1e-6 && (pq_code(10_000.0) - 1.0).abs() < 1e-4);
        assert!((pq_code(100.0) - 0.508).abs() < 0.002, "{}", pq_code(100.0));
        assert!((pq_code(203.0) - 0.581).abs() < 0.002, "{}", pq_code(203.0));
        assert!((pq_code(1000.0) - 0.752).abs() < 0.002, "{}", pq_code(1000.0));
        assert_eq!(pq_code(f32::INFINITY), pq_code(10_000.0));
    }
}
