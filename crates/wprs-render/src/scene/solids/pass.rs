use super::calls::Call;
use super::{BONES_SLOT, FINISH_SLOT, GpuModel, SCENE_SLOT, SLOT, Staged};
use crate::Renderer;
use std::collections::HashMap;
use wprs_scene::MAX_PLANES;

impl Renderer {
    pub(super) fn draw_calls<'p>(
        &self,
        pass: &mut wgpu::RenderPass<'p>,
        staged: &Staged,
        models: &HashMap<u64, GpuModel>,
        calls: impl Iterator<Item = (Call, super::pipes::Key)>,
    ) {
        let Some((_, groups)) = &staged.groups else { return };
        let solids = self.solids();
        let (mut key, mut bound) = (None, None);
        for (c, k) in calls {
            let (Some(gpu), Some(objects)) = (models.get(&c.model), staged.objects_groups.get(&c.model))
            else {
                continue;
            };
            if key != Some(k) {
                let Some(pipe) = solids
                    .pipe(&self.device, k)
                    .or_else(|| solids.pipe(&self.device, super::pipes::Key { package: 0, ..k }))
                else {
                    continue;
                };
                pass.set_pipeline(&pipe);
                key = Some(k);
            }
            if bound != Some(c.model) {
                pass.set_vertex_buffer(0, gpu.vertices.slice(..));
                pass.set_vertex_buffer(1, gpu.paint.slice(..));
                if let Some(skin) = &gpu.skin {
                    pass.set_vertex_buffer(2, skin.slice(..));
                }
                pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
                bound = Some(c.model);
            }
            let offsets = [c.object * SLOT as u32, c.bones * BONES_SLOT as u32, c.draw * SLOT as u32];
            pass.set_bind_group(1, objects, &offsets);
            pass.set_bind_group(2, &gpu.maps[c.maps.min(gpu.maps.len() - 1)], &[]);
            pass.set_bind_group(3, &groups[1], &[c.finish * FINISH_SLOT as u32]);
            let shells = if k.shadow { 1 } else { c.shells.max(1) };
            pass.draw_indexed(c.first..c.first + c.count, 0, 0..shells);
        }
    }

    pub(super) fn stage_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        staged: &Staged,
        models: &HashMap<u64, GpuModel>,
        (k, eye): (usize, usize),
    ) {
        let Some((_, groups)) = &staged.groups else { return };
        let target = if eye == 0 { &staged.slices[k] } else { &staged.cross[k] };
        let color = wgpu::RenderPassColorAttachment {
            view: &target.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        };
        let depth = wgpu::RenderPassDepthStencilAttachment {
            view: &staged.depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render solids"),
            color_attachments: &[Some(color)],
            depth_stencil_attachment: Some(depth),
            ..Default::default()
        });
        let slot = (eye * (MAX_PLANES + 1) + k) as u64 * SCENE_SLOT;
        pass.set_bind_group(0, &groups[0], &[slot as u32]);
        self.draw_calls(&mut pass, staged, models, staged.calls.iter().map(|c| (*c, c.key)));
    }
}
