use super::pool::{Pool, Tex};
use super::state::Sim;
use super::{Cached, Src};
use crate::{GpuImage, Renderer};
use std::collections::HashMap;
use wgpu::BindGroup;

pub(crate) struct Sources<'a> {
    pub(crate) caches: &'a HashMap<usize, Cached>,
    pub(crate) states: &'a HashMap<usize, Sim>,
    pub(crate) luts: &'a [(String, Tex)],
    pub(crate) exts: &'a [GpuImage],
    pub(crate) canvas: &'a Tex,
    pub(crate) output: &'a GpuImage,
    pub(crate) swarms: Option<&'a super::swarm::Swarms>,
    pub(crate) scissor: Option<[u32; 4]>,
    pub(crate) sets: &'a [[Src; 11]],
}

impl Sources<'_> {
    pub(crate) fn clip(&self, pass: &mut wgpu::RenderPass<'_>, target: Src) {
        if let Some([x, y, w, h]) = self.scissor.filter(|_| target == Src::Canvas) {
            pass.set_scissor_rect(x, y, w, h);
        }
    }

    pub(crate) fn get(&self, pool: &Pool, src: Src) -> (u64, wgpu::TextureView, wgpu::Texture) {
        let tex = |t: &Tex| (t.id, t.view.clone(), t.texture.clone());
        match src {
            Src::Pool(i) => tex(&pool.busy[i].0),
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

    pub(crate) fn own(&self, r: &Renderer, pool: &mut Pool, tex: &[Src; 11]) -> BindGroup {
        let views: [(u64, wgpu::TextureView); 11] = std::array::from_fn(|k| {
            let (id, view, _) = self.get(pool, tex[k]);
            (id, view)
        });
        let refs: [(u64, &wgpu::TextureView); 11] = std::array::from_fn(|k| (views[k].0, &views[k].1));
        pool.own(&r.device, &r.scene.get().expect("created with the view").own, &refs)
    }

    pub(crate) fn lit(&self, r: &Renderer, pool: &mut Pool, maps: &[Src; 8]) -> BindGroup {
        let views: [(u64, wgpu::TextureView); 8] = std::array::from_fn(|k| {
            let (id, view, _) = self.get(pool, maps[k]);
            (id, view)
        });
        let refs: [(u64, &wgpu::TextureView); 8] = std::array::from_fn(|k| (views[k].0, &views[k].1));
        pool.lit(&r.device, &r.scene.get().expect("created with the view").lit, &refs)
    }

    pub(crate) fn group(&self, r: &Renderer, pool: &mut Pool, fx: bool, tex: &[Src]) -> BindGroup {
        let views: [(u64, wgpu::TextureView); 7] = std::array::from_fn(|k| {
            let (id, view, _) = self.get(pool, tex[k.min(tex.len() - 1)]);
            (id, view)
        });
        let refs: [(u64, &wgpu::TextureView); 7] = std::array::from_fn(|k| (views[k].0, &views[k].1));
        let pipes = r.scene.get().expect("created with the view");
        pool.group(&r.device, if fx { &pipes.fx } else { &pipes.textures }, fx, &refs[..tex.len()])
    }
}
