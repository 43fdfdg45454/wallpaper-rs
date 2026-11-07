use super::super::SceneView;
use super::calls::{plan, put, rooms, sized};
use super::uniform::scene3;
use super::{GpuModel, SCENE_SLOT, Staged};
use crate::{RenderError, Renderer};
use std::collections::HashMap;
use wprs_scene::{Frame, MAX_PLANES, Stage3d};

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
        self.solid_stages(view, (&runtime.scene, frame), reach)
    }

    pub(crate) fn solid_stages(
        &self,
        view: &mut SceneView,
        (scene, frame): (&wprs_scene::Scene, &Frame<'_>),
        scale: f32,
    ) -> Result<(), RenderError> {
        let edge = self.max_edge();
        let hdr = view.hdr;
        for stage in &frame.stages {
            let size = sized(stage.size, scale, frame, edge);
            let room = rooms(frame, stage);
            let fits = view.solids.get(stage.layer).is_some_and(|s| {
                s.size == size
                    && s.slices.len() == stage.slices
                    && s.room >= room.0
                    && s.draw_room >= room.1
                    && s.bone_room >= room.2
                    && s.hdr == hdr
            });
            if !fits {
                view.solids.insert(stage.layer.to_string(), self.staged(size, stage.slices, hdr, room));
            }
            let key = stage.key ^ (u64::from(view.debug3d) << 56);
            if view.solids.get(stage.layer).is_some_and(|s| s.key == key && s.renders > 0) {
                continue;
            }
            for p in &frame.placed[stage.models.clone()] {
                if let std::collections::hash_map::Entry::Vacant(e) = view.models.entry(p.model.digest) {
                    e.insert(self.scoped(|| self.gpu_model(p.model))?);
                }
            }
            let looks = &frame.looks;
            self.packages(scene, looks.iter().map(|l| l.package).filter(|k| *k != 0));
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
            let flags = (map.is_some(), view.debug3d);
            let done = self.scoped(|| {
                self.paint_stage(staged, (&view.models, env_view, &view.uniform), frame, stage, flags)
            })?;
            view.pool.defer(done);
            staged.key = key;
            staged.renders += 1;
        }
        let live: Vec<&str> = frame.stages.iter().map(|s| s.layer).collect();
        view.solids.retain(|k, _| live.contains(&k.as_str()));
        view.models.retain(|d, _| frame.placed.iter().any(|p| p.model.digest == *d));
        view.envs.retain(|k, _| frame.placed.iter().any(|p| p.env == Some(k.as_str())));
        Ok(())
    }

    fn paint_stage(
        &self,
        staged: &mut Staged,
        (models, env, uniform): (&HashMap<u64, GpuModel>, &crate::texture::GpuImage, &wgpu::Buffer),
        frame: &Frame<'_>,
        stage: &Stage3d<'_>,
        flags: (bool, u8),
    ) -> wgpu::CommandBuffer {
        plan(staged, frame, stage);
        let shadows = self.shadow_plan(frame, stage);
        let mut bytes = std::mem::take(&mut staged.bytes);
        bytes.clear();
        let eyes = [Some(stage.eye), stage.cross.map(|c| c.0)];
        for (e, eye) in eyes.iter().enumerate() {
            let Some(eye) = eye else { continue };
            for k in 0..stage.slices {
                let mut s = scene3(frame, (stage, eye), k, flags, self.light_cap());
                shadows
                    .iter()
                    .flatten()
                    .enumerate()
                    .for_each(|(i, p)| s.lights[p.lamp].more[2] = i as f32 + 1.0);
                put(&mut bytes, (e * (MAX_PLANES + 1) + k) as u64 * SCENE_SLOT, &s);
            }
        }
        staged.bytes = bytes;
        self.shadow_slots(staged, (frame, stage), &shadows, flags);
        self.queue.write_buffer(&staged.scene, 0, &staged.bytes);
        for (buffer, data) in [
            (&staged.objects, &staged.objects_bytes),
            (&staged.finishes, &staged.finish_bytes),
            (&staged.bones, &staged.bone_bytes),
            (&staged.draws, &staged.draw_bytes),
        ] {
            if !data.is_empty() {
                self.queue.write_buffer(buffer, 0, data);
            }
        }
        let due = self.shadow_maps(staged, frame, stage, &shadows);
        if staged.groups.is_none() {
            staged.cross_groups.clear();
        }
        self.stage_groups(staged, env, uniform);
        for p in &frame.placed[stage.models.clone()] {
            if let Some(gpu) = models.get(&p.model.digest) {
                self.object_group(staged, p.model.digest, gpu);
            }
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.shadow_passes(&mut encoder, staged, models, due);
        for k in 0..stage.slices {
            self.stage_pass(&mut encoder, staged, models, (k, 0));
        }
        if let Some((_, f)) = stage.cross {
            if staged.cross.len() != stage.slices {
                staged.cross = self.slice_images(staged.size, staged.hdr, stage.slices);
                staged.cross_groups.clear();
            }
            if staged.cross_groups.len() != stage.slices {
                staged.cross_groups = staged.cross.iter().map(|c| self.mix_group(c)).collect();
            }
            for k in 0..stage.slices {
                self.stage_pass(&mut encoder, staged, models, (k, 1));
                self.mix_slices(&mut encoder, &staged.cross_groups[k], &staged.slices[k], f);
            }
        }
        encoder.finish()
    }
}
