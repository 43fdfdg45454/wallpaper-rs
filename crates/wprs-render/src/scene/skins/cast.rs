use super::super::SceneView;
use super::{PIECES, Skin};
use crate::texture::GpuImage;
use crate::{RenderError, Renderer};
use wprs_scene::{Content, Frame, Posed, Quality, Relief, Rigged, Runtime};

fn shrink(quality: Quality) -> f32 {
    match quality {
        Quality::Q0 => 0.5,
        Quality::Q1 => 0.75,
        _ => 1.0,
    }
}

impl Renderer {
    pub(crate) fn skins(
        &self,
        view: &mut SceneView,
        (runtime, frame): (&Runtime, &Frame<'_>),
        videos: &[(&str, &GpuImage)],
        scale: f32,
    ) -> Result<(), RenderError> {
        for d in &frame.draws {
            let Content::Mesh { layer, size, pose, texture, relief } = &d.content else { continue };
            let Some(p) = runtime.puppet(layer) else { continue };
            let edge = self.max_edge();
            let [w, h] = size.map(|v| ((v * scale * shrink(frame.quality)).round() as u32).clamp(1, edge));
            let stale =
                view.skins.get(*layer).is_none_or(|s| s.digest != p.rig.digest || s.image.size() != (w, h));
            if stale {
                let made = self.fresh_skin(&p.rig, (w, h))?;
                view.skins.insert(layer.to_string(), made);
            }
            let skin = view.skins.get_mut(*layer).expect("made above");
            let named = texture.as_deref();
            let video = named.and_then(|t| videos.iter().find(|(n, _)| *n == t)).map(|(_, v)| *v);
            let picture = video
                .map(|v| (v.id, v.view.clone()))
                .or_else(|| named.and_then(|t| view.images.get(t)).map(|i| (i.0.id, i.0.view.clone())))
                .or_else(|| skin.own.as_ref().map(|o| (o.id, o.view.clone())))
                .unwrap_or_else(|| (view.pool.white.id, view.pool.white.view.clone()));
            if skin.generation == *pose
                && skin.relief == *relief
                && video.is_none()
                && skin.group.as_ref().is_some_and(|g| g.0 == picture.0)
            {
                continue;
            }
            self.bind_skin(skin, (picture.0, &picture.1));
            self.scoped(|| self.paint_skin(skin, (&p.rig, p.posed()), *size, *relief))?;
            (skin.generation, skin.relief) = (*pose, *relief);
        }
        view.skins.retain(|k, _| runtime.puppet(k).is_some());
        Ok(())
    }

    fn paint_skin(
        &self,
        skin: &mut Skin,
        (rig, posed): (&Rigged, &Posed),
        [w, h]: [f32; 2],
        relief: Option<Relief>,
    ) {
        self.queue.write_buffer(&skin.bones, 0, bytemuck::cast_slice(&posed.palette));
        skin.scratch.clear();
        skin.scratch.push([w, h, 0.0, 0.0]);
        let [x, y, z] = relief.map_or([0.0; 3], |r| r.lift);
        skin.scratch.push([x, y, z, 0.0]);
        for (piece, slots) in rig.pieces.iter().zip(&posed.morphs) {
            skin.scratch.push([piece.base as f32, 0.0, 0.0, 0.0]);
            let pairs = slots.chunks(2).map(|p| [p[0].0 as f32, p[0].1, p[1].0 as f32, p[1].1]);
            skin.scratch.extend(pairs);
            skin.scratch.push(piece.color);
        }
        skin.scratch.truncate(PIECES);
        self.queue.write_buffer(&skin.pieces, 0, bytemuck::cast_slice(&skin.scratch));
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let color = wgpu::RenderPassColorAttachment {
            view: &skin.image.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        };
        let depth = wgpu::RenderPassDepthStencilAttachment {
            view: &skin.depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0),
                store: wgpu::StoreOp::Discard,
            }),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render skin"),
            color_attachments: &[Some(color)],
            depth_stencil_attachment: Some(depth),
            ..Default::default()
        });
        let skinner = self.skinner();
        if let Some((_, group)) = &skin.group {
            pass.set_bind_group(0, group, &[]);
            pass.set_vertex_buffer(0, skin.vertices.slice(..));
            pass.set_index_buffer(skin.indices.slice(..), wgpu::IndexFormat::Uint32);
            let (mut clipped, cull) = (None, 2 * usize::from(relief.is_some_and(|r| r.cull)));
            for d in &posed.draws {
                let Some(piece) = rig.pieces.get(d.piece as usize) else { continue };
                if clipped != Some(d.clip.is_some()) {
                    clipped = Some(d.clip.is_some());
                    pass.set_pipeline(&skinner.pipes[usize::from(d.clip.is_some()) + cull]);
                }
                pass.set_stencil_reference(u32::from(d.clip.unwrap_or(d.part)) % 255 + 1);
                pass.draw_indexed(piece.first..piece.first + piece.count, 0, 0..1);
            }
        }
        drop(pass);
        self.queue.submit([encoder.finish()]);
    }
}
