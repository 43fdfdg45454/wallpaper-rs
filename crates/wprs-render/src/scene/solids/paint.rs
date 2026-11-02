use super::super::SceneView;
use super::calls::{Call, put, sized};
use super::uniform::scene3;
use super::{SCENE_SLOT, SLOT, Staged};
use crate::{RenderError, Renderer};
use wprs_scene::{Frame, Stage3d};

impl Renderer {
    pub(crate) fn figures(
        &self,
        view: &mut SceneView,
        (runtime, frame): (&wprs_scene::Runtime, &Frame<'_>),
        videos: &[(&str, &crate::texture::GpuImage)],
        scale: [f32; 6],
    ) -> Result<(), RenderError> {
        let reach = scale[0].abs().max(scale[3].abs());
        self.skins(view, (runtime, frame), videos, reach)?;
        self.solid_stages(view, frame, reach)
    }

    pub(crate) fn solid_stages(
        &self,
        view: &mut SceneView,
        frame: &Frame<'_>,
        scale: f32,
    ) -> Result<(), RenderError> {
        let edge = self.max_edge();
        let hdr = view.hdr;
        for stage in &frame.stages {
            let size = sized(stage.size, scale, frame, edge);
            let room: usize =
                frame.placed[stage.models.clone()].iter().map(|p| p.model.materials.len() + 1).sum();
            let fits = view.solids.get(stage.layer).is_some_and(|s| {
                s.size == size && s.slices.len() == stage.slices && s.room >= room && s.hdr == hdr
            });
            if !fits {
                view.solids.insert(stage.layer.to_string(), self.staged(size, stage.slices, hdr, room));
            }
            if view.solids.get(stage.layer).is_some_and(|s| s.key == stage.key && s.renders > 0) {
                continue;
            }
            for p in &frame.placed[stage.models.clone()] {
                if let std::collections::hash_map::Entry::Vacant(e) = view.models.entry(p.model.digest) {
                    e.insert(self.scoped(|| self.gpu_model(p.model))?);
                }
            }
            let env = frame.placed[stage.models.clone()].iter().find_map(|p| p.env);
            let source = env.and_then(|e| view.images.get(e)).map(|i| &i.0);
            if let (Some(e), Some(source)) = (env, source)
                && view.envs.get(e).is_none_or(|(id, _)| *id != source.id)
            {
                let made = self.env_map(source);
                view.envs.insert(e.to_string(), (source.id, made));
            }
            let map = env.and_then(|e| view.envs.get(e)).map(|(_, m)| m);
            let staged = view.solids.get_mut(stage.layer).expect("made above");
            let env_view = map.unwrap_or(&self.solids().white);
            self.stage_groups(staged, env_view);
            let models = &view.models;
            self.scoped(|| self.paint_stage(staged, models, frame, stage, map.is_some()))?;
            staged.key = stage.key;
            staged.renders += 1;
        }
        let live: Vec<&str> = frame.stages.iter().map(|s| s.layer).collect();
        view.solids.retain(|k, _| live.contains(&k.as_str()));
        Ok(())
    }

    fn paint_stage(
        &self,
        staged: &mut Staged,
        models: &std::collections::HashMap<u64, super::GpuModel>,
        frame: &Frame<'_>,
        stage: &Stage3d<'_>,
        env: bool,
    ) {
        let mut bytes = std::mem::take(&mut staged.bytes);
        bytes.clear();
        for k in 0..stage.slices {
            put(&mut bytes, k as u64 * SCENE_SLOT, &scene3(frame, stage, k, env, self.light_cap()));
        }
        self.queue.write_buffer(&staged.scene, 0, &bytes);
        let (mut calls, mut objects, mut finishes) = (
            std::mem::take(&mut staged.calls),
            std::mem::take(&mut staged.objects_bytes),
            std::mem::take(&mut staged.finish_bytes),
        );
        calls.clear();
        objects.clear();
        finishes.clear();
        let mut slot = 0u32;
        for (o, p) in frame.placed[stage.models.clone()].iter().enumerate() {
            let (object, flipped) = super::looks::object(p);
            put(&mut objects, o as u64 * SLOT, &object);
            let looks = &frame.looks[p.looks.clone()];
            let far = stage.eye.depth([p.matrix[12], p.matrix[13], p.matrix[14]]);
            let defaults = wprs_scene::Finish::default();
            for (m, f) in p.model.materials.iter().chain([&defaults]).enumerate() {
                let look = looks.iter().find(|l| usize::from(l.material) == m);
                let (uniform, key) = super::looks::finish(f, look, p.opacity, flipped, staged.hdr);
                put(&mut finishes, u64::from(slot + m as u32) * SLOT, &uniform);
                for prim in p
                    .model
                    .primitives
                    .iter()
                    .filter(|q| q.material.map_or(p.model.materials.len(), usize::from) == m)
                {
                    let (first, count, maps, object, finish) =
                        (prim.first, prim.count, m, o as u32, slot + m as u32);
                    calls.push(Call { model: p.model.digest, first, count, maps, object, finish, key, far });
                }
            }
            slot += p.model.materials.len() as u32 + 1;
        }
        self.queue.write_buffer(&staged.objects, 0, &objects);
        self.queue.write_buffer(&staged.finishes, 0, &finishes);
        calls.sort_by(|a, b| {
            (a.key.blend > 0).cmp(&(b.key.blend > 0)).then(match a.key.blend > 0 {
                true => b.far.total_cmp(&a.far),
                false => std::cmp::Ordering::Equal,
            })
        });
        let Some((_, groups)) = &staged.groups else {
            (staged.bytes, staged.calls, staged.objects_bytes, staged.finish_bytes) =
                (bytes, calls, objects, finishes);
            return;
        };
        let solids = self.solids();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for k in 0..stage.slices {
            let color = wgpu::RenderPassColorAttachment {
                view: &staged.slices[k].view,
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
            pass.set_bind_group(0, &groups[0], &[(k as u64 * SCENE_SLOT) as u32]);
            let (mut key, mut bound) = (None, None);
            for c in &calls {
                let Some(gpu) = models.get(&c.model) else { continue };
                if key != Some(c.key) {
                    pass.set_pipeline(&solids.pipe(&self.device, c.key));
                    key = Some(c.key);
                }
                if bound != Some(c.model) {
                    pass.set_vertex_buffer(0, gpu.vertices.slice(..));
                    pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
                    bound = Some(c.model);
                }
                pass.set_bind_group(1, &groups[1], &[c.object * SLOT as u32]);
                pass.set_bind_group(2, &gpu.maps[c.maps.min(gpu.maps.len() - 1)], &[]);
                pass.set_bind_group(3, &groups[2], &[c.finish * SLOT as u32]);
                pass.draw_indexed(c.first..c.first + c.count, 0, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        (staged.bytes, staged.calls, staged.objects_bytes, staged.finish_bytes) =
            (bytes, calls, objects, finishes);
    }
}
