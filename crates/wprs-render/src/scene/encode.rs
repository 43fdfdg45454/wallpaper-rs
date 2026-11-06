use super::clock::{Clock, Timing};
use super::pipes::{Mode, Shader};
use super::pool::Pool;
use super::{Op, SceneStats, Src};
use crate::Renderer;
use std::time::Instant;

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
        resolve: Option<(u32, &'static str)>,
        (clock, timing): (&Clock, Timing),
        (view, serial, timed): (usize, u64, bool),
    ) -> (SceneStats, wgpu::SubmissionIndex, Instant) {
        let pipes = self.scene.get().expect("created with the view");
        let mut encoder = self.device.create_command_encoder(&Default::default());
        clock.open(&mut encoder, timing);
        let mut stats = SceneStats::default();
        let frame = pool.frame_groups().clone();
        let (mut k, mut profile) = (0, super::profile::Profile::begin(&self.device, timed));
        while k < ops.len() {
            if let Some(p) = profile.as_mut() {
                let (op, target) = (&ops[k], super::profile::target(&ops[k]));
                let texture = target.map(|t| src.get(pool, t).2);
                let shape = texture.map_or(((0, 0), wgpu::TextureFormat::R8Unorm), |t| {
                    ((t.width(), t.height()), t.format())
                });
                p.start(&mut encoder, || super::profile::label(op, shape.0, shape.1));
            }
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
                        if let Some([x, y, w, h]) = f.scissor {
                            pass.set_scissor_rect(x, y, w, h);
                        }
                        pass.draw(0..f.vertices, 0..1);
                    }
                    (stats.passes, stats.draws, k) = (stats.passes + 1, stats.draws + 1, k + 1);
                }
                Op::Strip { from } => {
                    let texture = src.get(pool, *from).2;
                    pool.strip.read(self, &mut encoder, &texture);
                    k += 1;
                }
                Op::Read { from, map, size } => {
                    let texture = src.get(pool, *from).2;
                    src.swarms.iter().for_each(|s| s.read(&mut encoder, &texture, *map, *size));
                    k += 1;
                }
                Op::Clear(target, _)
                | Op::Draw { target, .. }
                | Op::Lit { target, .. }
                | Op::Particles { target, .. } => {
                    let (target, first) = (*target, k);
                    let load = match &ops[k] {
                        Op::Clear(_, [r, g, b, a]) => {
                            k += 1;
                            wgpu::LoadOp::Clear(wgpu::Color { r: *r, g: *g, b: *b, a: *a })
                        }
                        _ => wgpu::LoadOp::Load,
                    };
                    let writes = if k == first { clock.writes(k) } else { None };
                    let alone = writes.is_some();
                    let (_, view, texture) = src.get(pool, target);
                    let mut pass = pass(&mut encoder, &view, load, writes);
                    src.clip(&mut pass, target);
                    while let Some(op) = ops.get(k).filter(|op| super::profile::target(op) == Some(target)) {
                        if k > first && (alone || clock.writes(k).is_some()) {
                            break;
                        }
                        let (shader, slot, mode, tex) = match op {
                            Op::Particles { system, slot, mode, bend, .. } => {
                                src.swarms
                                    .iter()
                                    .for_each(|s| s.draw(&mut pass, *system, *slot, (*mode, *bend)));
                                (stats.draws, k) = (stats.draws + 1, k + 1);
                                continue;
                            }
                            Op::Draw { entry, mode, slot, tex, .. } => {
                                (Shader::Scene(entry), slot, mode, tex)
                            }
                            Op::Lit { entry, mode, slot, tex, .. } => (Shader::Lit(entry), slot, mode, tex),
                            _ => break,
                        };
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
            profile.iter_mut().for_each(|p| p.end(&mut encoder));
        }
        let blit = |e, p| pipes.get(&self.device, Shader::Scene(e), Mode::Replace, p);
        if let Some((slot, pipeline)) =
            resolve.and_then(|(slot, entry)| Some((slot, blit(entry, src.output.texture.format())?)))
        {
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
        let profiled = profile.is_some();
        if let Some(p) = profile {
            p.finish(&self.device, &self.queue, &mut encoder);
        }
        clock.close(&mut encoder, timing);
        encoder.on_submitted_work_done(move || super::flight::done(view, serial));
        let at = Instant::now();
        let submitted = self.queue.submit([encoder.finish()]);
        if profiled {
            super::profile::submitted();
        }
        (stats, submitted, at)
    }
}
