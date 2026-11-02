use super::calls::put;
use super::uniform::{Scene3, Shadows, light_from, scene3};
use super::{DEPTH, GpuModel, SCENE_SLOT, Staged, staged::ShadowMap};
use crate::Renderer;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hasher};
use wgpu::TextureUsages as U;
use wprs_scene::{Beam, Frame, MAX_PLANES, Stage3d};

mod frusta;

pub(crate) use frusta::{Plan, plan_for};

const FIRST: usize = 2 * (MAX_PLANES + 1);

fn layered(t: &wgpu::Texture, layer: Option<u32>) -> wgpu::TextureView {
    t.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(match layer {
            Some(_) => wgpu::TextureViewDimension::D2,
            None => wgpu::TextureViewDimension::D2Array,
        }),
        base_array_layer: layer.unwrap_or(0),
        array_layer_count: layer.map(|_| 1),
        ..Default::default()
    })
}

fn depth_texture(r: &Renderer, side: u32, layers: u32, usage: U) -> wgpu::Texture {
    r.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wprs-render shadow map"),
        size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: layers },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH,
        usage,
        view_formats: &[],
    })
}

pub(super) fn blank(r: &Renderer) -> wgpu::TextureView {
    layered(&depth_texture(r, 1, 1, U::TEXTURE_BINDING), None)
}

impl Renderer {
    pub(super) fn shadow_plan(&self, frame: &Frame, stage: &Stage3d) -> [Option<Plan>; 2] {
        let phone = cfg!(target_os = "android");
        let cap = wprs_scene::model_caps(self.tier(), phone).shadows.min(2);
        let mut out = [None, None];
        let kinds = |l: &&wprs_scene::Lamp| matches!(l.beam, Beam::Point | Beam::Spot | Beam::Directional);
        let lit = frame.lamps.iter().take(self.light_cap()).enumerate();
        let cast = lit.filter(|(_, l)| l.shadow.is_some() && kinds(l));
        for ((lamp, l), slot) in cast.take(cap).zip(&mut out) {
            let set = l.shadow.and_then(|s| s.0);
            let side = wprs_scene::shadow_side(l.beam, set, (self.tier(), phone), frame.quality);
            if side > 0 {
                let light = light_from(stage, &stage.eye, l);
                *slot = Some(plan_for(frame, stage, (lamp, l, &light), side));
            }
        }
        out
    }

    pub(super) fn shadow_slots(
        &self,
        staged: &mut Staged,
        (frame, stage): (&Frame, &Stage3d),
        plans: &[Option<Plan>; 2],
        flags: (bool, u8),
    ) {
        let mut shades = Shadows::default();
        let base: Scene3 = scene3(frame, (stage, &stage.eye), 0, flags, self.light_cap());
        for (s, p) in plans.iter().enumerate() {
            let Some(p) = p else { continue };
            shades.info[s] = [p.layers as f32, (s * 6) as f32, 1.0 / p.side as f32, p.bias];
            shades.at[s] = [p.at[0], p.at[1], p.at[2], p.texel * 4.0];
            for f in 0..p.layers as usize {
                shades.mats[s * 6 + f] = p.mats[f];
                let face = Scene3 { clip: p.mats[f], ..base };
                put(&mut staged.bytes, (FIRST + s * 6 + f) as u64 * SCENE_SLOT, &face);
            }
        }
        self.queue.write_buffer(&staged.shades, 0, bytemuck::bytes_of(&shades));
    }

    pub(super) fn shadow_maps(
        &self,
        staged: &mut Staged,
        frame: &Frame,
        stage: &Stage3d,
        plans: &[Option<Plan>; 2],
    ) -> [bool; 2] {
        let mut due = [false; 2];
        for (s, plan) in plans.iter().enumerate() {
            let Some(p) = plan else {
                if staged.maps[s].take().is_some() {
                    staged.groups = None;
                }
                continue;
            };
            let mut h = DefaultHasher::new();
            p.mats.iter().flatten().for_each(|x| h.write_u32(x.to_bits()));
            for q in &frame.placed[stage.models.clone()] {
                h.write_u64(q.model.digest);
                h.write_u64(q.pose.map_or(0, |x| x.generation));
                q.matrix.iter().for_each(|x| h.write_u32(x.to_bits()));
                let _ = std::fmt::Write::write_fmt(
                    &mut Hashed(&mut h),
                    format_args!("{:?}", &frame.looks[q.looks.clone()]),
                );
            }
            let swaying = stage.wind != [0.0; 2];
            [stage.wind[0], stage.wind[1], if swaying { stage.time } else { 0.0 }]
                .iter()
                .for_each(|x| h.write_u32(x.to_bits()));
            let key = h.finish();
            if !staged.maps[s].as_ref().is_some_and(|m| m.side == p.side && m.layers == p.layers) {
                let texture =
                    depth_texture(self, p.side, p.layers, U::TEXTURE_BINDING | U::RENDER_ATTACHMENT);
                let view = layered(&texture, None);
                staged.maps[s] =
                    Some(ShadowMap { texture, view, side: p.side, layers: p.layers, key: 0, renders: 0 });
                staged.groups = None;
            }
            let map = staged.maps[s].as_mut().expect("made above");
            if map.key != key || map.renders == 0 {
                (map.key, map.renders, due[s]) = (key, map.renders + 1, true);
                staged.shadow_renders += 1;
            }
        }
        due
    }

    pub(super) fn shadow_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        staged: &Staged,
        models: &HashMap<u64, GpuModel>,
        due: [bool; 2],
    ) {
        let Some((_, groups)) = &staged.groups else { return };
        for (s, map) in staged.maps.iter().enumerate().filter(|(s, _)| due[*s]) {
            let Some(map) = map else { continue };
            for f in 0..map.layers {
                let view = layered(&map.texture, Some(f));
                let depth = wgpu::RenderPassDepthStencilAttachment {
                    view: &view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                };
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("wprs-render shadows"),
                    depth_stencil_attachment: Some(depth),
                    ..Default::default()
                });
                let slot = (FIRST + s * 6 + f as usize) as u64 * SCENE_SLOT;
                pass.set_bind_group(0, &groups[2], &[slot as u32]);
                let casters = staged.calls.iter().filter(|c| c.cast);
                let keyed = casters.map(|c| {
                    let k = super::pipes::Key {
                        shadow: true,
                        package: 0,
                        blend: 0,
                        write: true,
                        test: true,
                        cull: 0,
                        ..c.key
                    };
                    (*c, k)
                });
                self.draw_calls(&mut pass, staged, models, keyed);
            }
        }
    }
}

struct Hashed<'h>(&'h mut DefaultHasher);

impl std::fmt::Write for Hashed<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}
