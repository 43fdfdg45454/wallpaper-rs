use super::super::item::Item;
use super::super::pool::{Pool, Tex};
use super::super::{SceneView, Scratch};
use crate::Renderer;
use crate::texture::GpuImage;
use wgpu::TextureFormat as Format;
use wprs_scene::Runtime;

pub(super) fn retarget(r: &Renderer, view: &mut SceneView, size: (u32, u32)) {
    let pool = &mut view.pool;
    view.caches.drain().for_each(|(_, c)| pool.give(c.tex));
    if let Some((canvas, output)) = view.target.take() {
        let GpuImage { texture, view, id } = output;
        if id != canvas.id {
            pool.give(Tex { texture, view, id });
        }
        pool.give(canvas);
    }
    let canvas = pool.take(&r.device, size, view.format);
    let output = match view.srgb {
        true => {
            let Tex { texture, view, id } = pool.take(&r.device, size, Format::Rgba8UnormSrgb);
            GpuImage { texture, view, id }
        }
        false => GpuImage { texture: canvas.texture.clone(), view: canvas.view.clone(), id: canvas.id },
    };
    view.target = Some((canvas, output));
    view.marks.clear();
}

pub(super) fn resolve(pool: &mut Pool) -> u32 {
    let (m0, m1, full) = ([2.0, 0.0, -1.0, 0.0], [0.0, -2.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
    let mode = [0.0, 1.0, 0.0, 0.0];
    pool.push(&Item { m0, m1, quad: full, uv: full, color: [1.0; 4], mode, ..Default::default() })
}

impl Scratch {
    pub(super) fn reset(&mut self, draws: usize, runtime: &Runtime) {
        self.ops.clear();
        self.exts.clear();
        self.kept.clear();
        self.outputs.clear();
        self.texts.clear();
        self.drawn.clear();
        self.sets.clear();
        self.made.clear();
        self.isolated.clear();
        self.chains.clear();
        self.chains.resize(draws, None);
        self.flocks.clear();
        self.flocks.extend((0..).map_while(|i| runtime.swarm(i).map(|w| w.stamp ^ u64::from(w.epoch) << 40)));
    }
}
