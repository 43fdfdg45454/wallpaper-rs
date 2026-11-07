use super::Skin;
use crate::Renderer;
use std::collections::HashMap;
use wprs_scene::Puppet;

pub(crate) const WAVE: usize = 4;

type Member<'a> = (&'a str, u64, wgpu::TextureView);

#[derive(Default)]
pub(super) struct Wave<'a> {
    lead: Option<(&'a Puppet, [f32; 2], (u32, u32))>,
    skins: [Option<Member<'a>>; WAVE],
    count: usize,
}

impl<'a> Wave<'a> {
    pub(super) fn joins(&self, p: &Puppet, size: [f32; 2], fits: (u32, u32)) -> bool {
        let Some((lead, s, f)) = self.lead else { return false };
        let (a, b) = (lead.posed(), p.posed());
        let same = a.palette == b.palette && a.morphs == b.morphs && a.draws == b.draws;
        self.count < WAVE && lead.rig.digest == p.rig.digest && s == size && f == fits && same
    }

    pub(super) fn waiting(&self) -> bool {
        self.lead.is_some()
    }

    pub(super) fn start(&mut self, p: &'a Puppet, size: [f32; 2], fits: (u32, u32)) {
        self.lead = Some((p, size, fits));
    }

    pub(super) fn add(&mut self, layer: &'a str, (id, view): (u64, wgpu::TextureView)) {
        self.skins[self.count] = Some((layer, id, view));
        self.count += 1;
    }
}

impl Renderer {
    pub(super) fn wave(
        &self,
        skins: &mut HashMap<String, Skin>,
        wave: &mut Wave<'_>,
    ) -> Option<wgpu::CommandBuffer> {
        let (lead, size, _) = wave.lead.take()?;
        let count = std::mem::take(&mut wave.count);
        let members = wave.skins.each_mut().map(Option::take);
        let first = members[0].as_ref()?.0;
        let skin = skins.get_mut(first)?;
        if count == 1 {
            return Some(self.paint_skin(skin, (&lead.rig, lead.posed()), size, None));
        }
        self.pose_skin(skin, (&lead.rig, lead.posed()), size, None);
        let key = members.each_ref().map(|m| m.as_ref().map_or(0, |m| m.1));
        if skin.merged.as_ref().is_none_or(|(k, _)| *k != key) {
            let views = members.each_ref().map(|m| m.as_ref().map(|m| &m.2));
            skin.merged = self.wave_group(skin, views).map(|g| (key, g));
        }
        let skin = skins.get(first)?;
        let attach = |m: &Option<Member<'_>>| {
            let view = &skins.get(m.as_ref()?.0)?.image.view;
            let ops = wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            };
            Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops })
        };
        let colors = members.each_ref().map(attach);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render skins"),
            color_attachments: &colors[..count],
            depth_stencil_attachment: Some(super::cast::cleared(&skin.depth)),
            ..Default::default()
        });
        if let Some((_, group)) = &skin.merged {
            pass.set_pipeline(&self.skinner().waves[count - 2]);
            pass.set_bind_group(0, group, &[]);
            pass.set_vertex_buffer(0, skin.vertices.slice(..));
            pass.set_index_buffer(skin.indices.slice(..), wgpu::IndexFormat::Uint32);
            for (order, d) in (0u32..).zip(&lead.posed().draws) {
                let Some(piece) = lead.rig.pieces.get(d.piece as usize) else { continue };
                pass.draw_indexed(piece.first..piece.first + piece.count, 0, order..order + 1);
            }
        }
        drop(pass);
        Some(encoder.finish())
    }

    fn wave_group(&self, skin: &Skin, views: [Option<&wgpu::TextureView>; WAVE]) -> Option<wgpu::BindGroup> {
        let lead = views[0]?;
        let picture = |k: usize| wgpu::BindingResource::TextureView(views[k].unwrap_or(lead));
        let entries = [
            wgpu::BindGroupEntry { binding: 0, resource: skin.bones.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: skin.pieces.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&skin.morphs) },
            wgpu::BindGroupEntry { binding: 3, resource: picture(0) },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&self.pipes().sampler),
            },
            wgpu::BindGroupEntry { binding: 5, resource: picture(1) },
            wgpu::BindGroupEntry { binding: 6, resource: picture(2) },
            wgpu::BindGroupEntry { binding: 7, resource: picture(3) },
        ];
        let layout = &self.skinner().wave;
        let desc = wgpu::BindGroupDescriptor { label: Some("wprs-render skins"), layout, entries: &entries };
        Some(self.device.create_bind_group(&desc))
    }
}
