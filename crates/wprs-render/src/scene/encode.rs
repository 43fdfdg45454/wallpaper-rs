use super::clock::{Clock, Timing};
use super::pipes::{Mode, Shader};
use super::pool::Pool;
use super::{Op, SceneStats, Src};
use crate::Renderer;
use std::time::Instant;
use wgpu::TextureFormat as Format;

pub(crate) use super::sources::Sources;

fn pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'e> {
    let ops = wgpu::Operations { load, store: wgpu::StoreOp::Store };
    let attachment = wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("wprs-scene pass"),
        color_attachments: &[Some(attachment)],
        timestamp_writes,
        ..Default::default()
    })
}

impl Renderer {
    pub(crate) fn encode_scene(
        &self,
        pool: &mut Pool,
        src: &Sources<'_>,
        ops: &[Op],
        resolve: Option<u32>,
        (clock, timing): (&Clock, Timing),
        (view, serial): (usize, u64),
    ) -> (SceneStats, wgpu::SubmissionIndex, Instant) {
        let pipes = self.scene.get().expect("created with the view");
        let mut encoder = self.device.create_command_encoder(&Default::default());
        clock.open(&mut encoder, timing);
        let mut stats = SceneStats::default();
        let frame = pool.frame_groups().clone();
        let mut k = 0;
        while k < ops.len() {
            match &ops[k] {
                Op::Copy { from, to, origin, size, at } => {
                    let ((_, _, from), (_, _, to)) = (src.get(pool, *from), src.get(pool, *to));
                    let place = |texture, [x, y]: [u32; 2]| wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    };
                    let extent = wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 };
                    encoder.copy_texture_to_texture(place(&from, *origin), place(&to, *at), extent);
                    k += 1;
                }
                Op::Effect(f) => {
                    let (_, view, texture) = src.get(pool, f.target);
                    let pipeline = pipes.get(&self.device, f.shader, f.mode, texture.format());
                    let writes = clock.writes(k);
                    let tex = &src.sets[f.tex as usize];
                    let group = match f.more {
                        Some(_) => src.own(self, pool, tex),
                        None => src.group(self, pool, true, &tex[..7]),
                    };
                    let load = if f.load {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    };
                    let mut pass = pass(&mut encoder, &view, load, writes);
                    if let Some(pipeline) = pipeline {
                        pass.set_pipeline(&pipeline);
                        pass.set_bind_group(0, &frame.1, &[]);
                        match f.more {
                            Some(more) => pass.set_bind_group(1, &group, &[f.slot, more]),
                            None => pass.set_bind_group(1, &group, &[f.slot]),
                        }
                        pass.draw(0..f.vertices, 0..1);
                    }
                    (stats.passes, stats.draws, k) = (stats.passes + 1, stats.draws + 1, k + 1);
                }
                Op::Read { from, map, size } => {
                    let texture = src.get(pool, *from).2;
                    src.swarms.iter().for_each(|s| s.read(&mut encoder, &texture, *map, *size));
                    k += 1;
                }
                Op::Particles { target, system, slot, mode, .. } => {
                    let view = src.get(pool, *target).1;
                    let mut pass = pass(&mut encoder, &view, wgpu::LoadOp::Load, clock.writes(k));
                    src.clip(&mut pass, *target);
                    src.swarms.iter().for_each(|s| s.draw(&mut pass, *system, *slot, *mode));
                    (stats.passes, stats.draws, k) = (stats.passes + 1, stats.draws + 1, k + 1);
                }
                Op::Clear(target, _) | Op::Draw { target, .. } | Op::Lit { target, .. } => {
                    let target = *target;
                    let load = match &ops[k] {
                        Op::Clear(_, [r, g, b, a]) => {
                            k += 1;
                            wgpu::LoadOp::Clear(wgpu::Color { r: *r, g: *g, b: *b, a: *a })
                        }
                        _ => wgpu::LoadOp::Load,
                    };
                    let (_, view, texture) = src.get(pool, target);
                    let mut pass = pass(&mut encoder, &view, load, None);
                    src.clip(&mut pass, target);
                    while let Some(
                        op @ (Op::Draw { target: t, entry, mode, slot, tex }
                        | Op::Lit { target: t, entry, mode, slot, tex, .. }),
                    ) = ops.get(k)
                    {
                        if *t != target {
                            break;
                        }
                        let shader =
                            if let Op::Lit { .. } = op { Shader::Lit(entry) } else { Shader::Scene(entry) };
                        let Some(pipeline) = pipes.get(&self.device, shader, *mode, texture.format()) else {
                            k += 1;
                            continue;
                        };
                        pass.set_pipeline(&pipeline);
                        pass.set_bind_group(0, &frame.0, &[*slot]);
                        pass.set_bind_group(1, &src.group(self, pool, false, tex), &[]);
                        if let Op::Lit { maps, .. } = op {
                            pass.set_bind_group(2, &src.lit(self, pool, maps), &[]);
                        }
                        pass.draw(0..4, 0..1);
                        (stats.draws, k) = (stats.draws + 1, k + 1);
                    }
                    stats.passes += 1;
                }
            }
        }
        let blit = |p| pipes.get(&self.device, Shader::Scene("fs_texture"), Mode::Replace, p);
        if let Some((slot, pipeline)) = resolve.and_then(|slot| Some((slot, blit(Format::Rgba8UnormSrgb)?))) {
            let group = src.group(self, pool, false, &[Src::Canvas, Src::White, Src::Clear]);
            let load =
                src.scissor.map_or(wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), |_| wgpu::LoadOp::Load);
            let mut pass = pass(&mut encoder, &src.output.view, load, None);
            src.clip(&mut pass, Src::Canvas);
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &frame.0, &[slot]);
            pass.set_bind_group(1, &group, &[]);
            pass.draw(0..4, 0..1);
            (stats.passes, stats.draws) = (stats.passes + 1, stats.draws + 1);
        }
        clock.close(&mut encoder, timing);
        encoder.on_submitted_work_done(move || super::flight::done(view, serial));
        let at = Instant::now();
        (stats, self.queue.submit([encoder.finish()]), at)
    }
}
