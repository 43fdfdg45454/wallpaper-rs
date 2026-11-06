use super::super::item::Item;
use super::super::pool::{Pool, Tex};
use super::super::{SceneView, Scratch};
use crate::Renderer;
use crate::texture::GpuImage;
use wgpu::TextureFormat as Format;
use wprs_scene::Runtime;

pub(super) fn retarget(r: &Renderer, view: &mut SceneView, size: (u32, u32)) {
    let wanted = output_format(view);
    let pool = &mut view.pool;
    view.caches.drain().for_each(|(_, c)| pool.give(c.tex));
    if let Some((canvas, output)) = view.target.take() {
        let GpuImage { texture, view, id, .. } = output;
        if id != canvas.id {
            pool.give(Tex { texture, view, id });
        }
        pool.give(canvas);
    }
    let views = r.gpu.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::VIEW_FORMATS);
    let viewed = views && wanted.remove_srgb_suffix() == view.format;
    let shared = view.tone == 0 && (wanted == view.format || viewed);
    let canvas = match shared && wanted != view.format {
        true => Tex::viewed(&r.device, size, (view.format, &[wanted])),
        false => pool.take(&r.device, size, view.format),
    };
    let output = match shared {
        false => {
            let Tex { texture, view, id } = pool.take(&r.device, size, wanted);
            GpuImage { texture, view, id, format: wanted }
        }
        true => {
            let format = Some(wanted);
            let view =
                canvas.texture.create_view(&wgpu::TextureViewDescriptor { format, ..Default::default() });
            GpuImage { texture: canvas.texture.clone(), view, id: canvas.id, format: wanted }
        }
    };
    view.target = Some((canvas, output));
    view.marks.clear();
}

pub(super) fn output_format(view: &SceneView) -> Format {
    match (view.srgb || view.tone > 0, view.tone > 0 && view.headroom > 1.0) {
        (true, true) => Format::Rgba16Float,
        (true, false) => Format::Rgba8UnormSrgb,
        (false, _) => view.format,
    }
}

pub(super) fn resolve(
    pool: &mut Pool,
    srgb: bool,
    (tone, split, lit): (u8, f32, f32),
) -> (u32, &'static str) {
    let (m0, m1, full) = ([2.0, 0.0, -1.0, 0.0], [0.0, -2.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
    let mode = [lit, f32::from(u8::from(srgb)), f32::from(tone), split.clamp(0.0, 1.0)];
    let slot = pool.push(&Item { m0, m1, quad: full, uv: full, color: [1.0; 4], mode, ..Default::default() });
    (slot, if tone > 0 { "fs_tonemap" } else { "fs_texture" })
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
