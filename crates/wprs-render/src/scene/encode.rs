use super::pipes::{Mode, Shader};
use super::pool::{Pool, Tex};
use super::state::Sim;
use super::{Cached, Op, SceneStats, Src};
use crate::{GpuImage, Renderer};
use std::collections::HashMap;
use wgpu::{BindGroup, RenderPipeline, TextureFormat as Format};

pub(crate) struct Sources<'a> {
    pub(crate) caches: &'a HashMap<usize, Cached>,
    pub(crate) states: &'a HashMap<usize, Sim>,
    pub(crate) luts: &'a [(String, Tex)],
    pub(crate) exts: &'a [&'a GpuImage],
    pub(crate) canvas: &'a Tex,
    pub(crate) output: &'a GpuImage,
    pub(crate) swarms: Option<&'a super::swarm::Swarms>,
}

impl Sources<'_> {
    fn get(&self, pool: &Pool, src: Src) -> (u64, wgpu::TextureView, wgpu::Texture) {
        let tex = |t: &Tex| (t.id, t.view.clone(), t.texture.clone());
        match src {
            Src::Pool(i) => tex(&pool.busy[i]),
            Src::Ext(i) => (self.exts[i].id, self.exts[i].view.clone(), self.exts[i].texture.clone()),
            Src::Cache(k) => self.caches.get(&k).map_or_else(|| tex(&pool.clear), |c| tex(&c.tex)),
            Src::White => tex(&pool.white),
            Src::Clear => tex(&pool.clear),
            Src::Canvas => tex(self.canvas),
            Src::State(k, side) => {
                self.states.get(&k).map_or_else(|| tex(&pool.clear), |s| tex(&s.tex[side]))
            }
            Src::Lut(i) => tex(&self.luts[i].1),
            Src::Void => tex(&pool.void),
        }
    }

    fn group(&self, r: &Renderer, pool: &mut Pool, fx: bool, tex: &[Src]) -> BindGroup {
        let views: Vec<(u64, wgpu::TextureView)> =
            tex.iter().map(|s| self.get(pool, *s)).map(|(id, v, _)| (id, v)).collect();
        let refs: Vec<(u64, &wgpu::TextureView)> = views.iter().map(|(id, v)| (*id, v)).collect();
        let pipes = r.scene.get().expect("created with the view");
        pool.group(&r.device, if fx { &pipes.fx } else { &pipes.textures }, fx, &refs)
    }
}

fn pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    let ops = wgpu::Operations { load, store: wgpu::StoreOp::Store };
    let attachment = wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("wprs-scene pass"),
        color_attachments: &[Some(attachment)],
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
    ) -> SceneStats {
        let pipes = self.scene.get().expect("created with the view");
        let mut encoder = self.device.create_command_encoder(&Default::default());
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
                Op::Effect { target, shader, slot, tex } => {
                    let (_, view, texture) = src.get(pool, *target);
                    let pipeline = pipes.get(&self.device, *shader, Mode::Replace, texture.format());
                    let group = src.group(self, pool, true, tex);
                    let mut pass = pass(&mut encoder, &view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT));
                    pass.set_pipeline(&pipeline);
                    pass.set_bind_group(0, &frame.1, &[]);
                    pass.set_bind_group(1, &group, &[*slot]);
                    pass.draw(0..3, 0..1);
                    (stats.passes, stats.draws, k) = (stats.passes + 1, stats.draws + 1, k + 1);
                }
                Op::Particles { target, system, slot, mode } => {
                    let view = src.get(pool, *target).1;
                    let mut pass = pass(&mut encoder, &view, wgpu::LoadOp::Load);
                    src.swarms.iter().for_each(|s| s.draw(&mut pass, *system, *slot, *mode));
                    (stats.passes, stats.draws, k) = (stats.passes + 1, stats.draws + 1, k + 1);
                }
                Op::Clear(target, _) | Op::Draw { target, .. } => {
                    let target = *target;
                    let load = match &ops[k] {
                        Op::Clear(_, [r, g, b, a]) => {
                            k += 1;
                            wgpu::LoadOp::Clear(wgpu::Color { r: *r, g: *g, b: *b, a: *a })
                        }
                        _ => wgpu::LoadOp::Load,
                    };
                    let mut calls: Vec<(RenderPipeline, BindGroup, u32)> = Vec::new();
                    let format = src.get(pool, target).2.format();
                    while let Some(Op::Draw { target: t, entry, mode, slot, tex }) = ops.get(k) {
                        if *t != target {
                            break;
                        }
                        calls.push((
                            pipes.get(&self.device, Shader::Scene(entry), *mode, format),
                            src.group(self, pool, false, tex),
                            *slot,
                        ));
                        k += 1;
                    }
                    let view = src.get(pool, target).1;
                    let mut pass = pass(&mut encoder, &view, load);
                    for (pipeline, group, slot) in &calls {
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(0, &frame.0, &[*slot]);
                        pass.set_bind_group(1, group, &[]);
                        pass.draw(0..4, 0..1);
                    }
                    (stats.passes, stats.draws) = (stats.passes + 1, stats.draws + calls.len() as u32);
                }
            }
        }
        if let Some(slot) = resolve {
            let pipeline =
                pipes.get(&self.device, Shader::Scene("fs_texture"), Mode::Replace, Format::Rgba8UnormSrgb);
            let group = src.group(self, pool, false, &[Src::Canvas, Src::White, Src::Clear]);
            let mut pass =
                pass(&mut encoder, &src.output.view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT));
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &frame.0, &[slot]);
            pass.set_bind_group(1, &group, &[]);
            pass.draw(0..4, 0..1);
            (stats.passes, stats.draws) = (stats.passes + 1, stats.draws + 1);
        }
        self.queue.submit([encoder.finish()]);
        stats
    }
}
