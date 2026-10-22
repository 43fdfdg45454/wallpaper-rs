mod chains;
mod draw;
mod effects;
mod encode;
mod item;
mod load;
mod paint;
mod pipes;
mod pool;
mod state;
mod swarm;
mod text;
mod view;

use crate::texture::{GpuImage, gpu_image};
use crate::{RenderError, Renderer};
use image::RgbaImage;
use item::{Rect, frame_data, linear};
use paint::Painter;
pub(crate) use pipes::Pipes;
use pipes::{Mode, Shader};
use pool::Tex;
use std::collections::HashMap;
use std::sync::Arc;
pub use view::SceneView;
use wgpu::TextureFormat as Format;
use wprs_scene::{Frame, Lut, Quality, Runtime};

const TEXT_FRAMES: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Src {
    Pool(usize),
    Ext(usize),
    Cache(usize),
    White,
    Clear,
    Canvas,
    State(usize, usize),
    Lut(usize),
    Void,
}

pub(crate) enum Op {
    Clear(Src, [f64; 4]),
    Copy { from: Src, to: Src, origin: [u32; 2], size: (u32, u32), at: [u32; 2] },
    Draw { target: Src, entry: &'static str, mode: Mode, slot: u32, tex: [Src; 3] },
    Effect { target: Src, shader: Shader, slot: u32, tex: [Src; 7] },
    Particles { target: Src, system: usize, slot: u32, mode: Mode },
}

pub(crate) struct Cached {
    key: u64,
    tex: Tex,
}

pub(crate) type Image = (GpuImage, [f32; 2]);

#[derive(Debug, Clone, Default)]
pub struct SceneAssets {
    pub images: HashMap<String, Arc<RgbaImage>>,
    pub fonts: HashMap<String, Arc<Vec<u8>>>,
    pub luts: HashMap<String, Arc<Lut>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SceneStats {
    pub passes: u32,
    pub draws: u32,
    pub created: u32,
    pub compiled: u32,
    pub textures: u32,
    pub reused: u32,
    pub steps: u32,
    pub particles: u32,
    pub sim_us: u32,
}

impl Renderer {
    pub fn software(&self) -> bool {
        self.gpu.get_info().device_type == wgpu::DeviceType::Cpu
    }

    pub fn discrete(&self) -> bool {
        self.gpu.get_info().device_type == wgpu::DeviceType::DiscreteGpu
    }

    pub fn render_scene<'v>(
        &self,
        view: &'v mut SceneView,
        runtime: &Runtime,
        frame: &Frame<'_>,
        size: (u32, u32),
        quality: Quality,
        videos: &[(&str, &GpuImage)],
    ) -> Result<&'v GpuImage, RenderError> {
        self.check_target(size.0, size.1)?;
        let created = (view.pool.created, self.pipes().created.load(std::sync::atomic::Ordering::Relaxed));
        if view.target.as_ref().is_none_or(|(t, _)| t.size() != size) {
            view.caches.clear();
            let canvas = Tex::new(&self.device, size, view.format);
            let output = match view.srgb {
                true => Tex::new(&self.device, size, Format::Rgba8UnormSrgb).texture,
                false => canvas.texture.clone(),
            };
            view.target = Some((canvas, gpu_image(output)));
            view.pool.created += 2;
        }
        let screen = view.screen(size);
        self.queue.write_buffer(
            &view.uniform,
            0,
            bytemuck::bytes_of(&frame_data(runtime, frame, size, view.srgb, quality)),
        );
        let texts = self.rasters(view, frame, screen);
        if let Some(swarms) = view.swarms.as_mut() {
            self.scoped(|| swarms.run(self, runtime, frame, quality))??;
        }
        let SceneView {
            pool,
            caches,
            images,
            texts: rasters,
            target,
            uniform,
            canvas,
            srgb,
            format,
            stats,
            luts,
            states,
            state_format,
            swarms,
            ..
        } = view;
        let rasters_of =
            |(i, key): (usize, u64)| rasters.get(&key).map(|(image, size, _)| (i, (image, *size)));
        let texts: HashMap<usize, (&GpuImage, [f32; 2])> = texts.into_iter().filter_map(rasters_of).collect();
        let full = Rect { x: 0.0, y: 0.0, w: size.0 as f32, h: size.1 as f32 };
        let (srgb, format, state_format) = (*srgb, *format, *state_format);
        let mut painter = Painter {
            r: self,
            frame,
            pool,
            caches,
            states,
            images,
            luts,
            texts: &texts,
            videos,
            exts: Vec::new(),
            screen,
            stage: screen.then(wprs_scene::Affine::scale(canvas.map(|v| v as f32))),
            full,
            srgb,
            format,
            state_format,
            ops: Vec::new(),
            reused: 0,
            kept: Vec::new(),
            swarms: swarms.as_mut(),
        };
        let c = frame.background.0;
        let background = if srgb { c } else { [linear(c[0]), linear(c[1]), linear(c[2]), c[3]] };
        painter.run(Src::Canvas, background.map(f64::from));
        let (ops, exts, reused, kept) = (painter.ops, painter.exts, painter.reused, painter.kept);
        caches.retain(|id, _| kept.contains(id));
        let resolve = srgb.then(|| {
            let (m0, m1) = ([2.0, 0.0, -1.0, 0.0], [0.0, -2.0, 1.0, 0.0]);
            let full = [0.0, 0.0, 1.0, 1.0];
            pool.push(&item::Item {
                m0,
                m1,
                quad: full,
                uv: full,
                color: [1.0; 4],
                mode: [0.0, 1.0, 0.0, 0.0],
                ..Default::default()
            })
        });
        pool.upload(&self.device, &self.queue, self.pipes(), uniform);
        swarms.iter_mut().for_each(|s| s.upload(self));
        let (canvas, output) = target.as_ref().expect("created above");
        let swarms = swarms.as_ref();
        let sources = encode::Sources { caches, states, luts, exts: &exts, canvas, output, swarms };
        let mut counted = self.scoped(|| self.encode_scene(pool, &sources, &ops, resolve))?;
        let painted = pool.frame;
        states.retain(|_, sim| painted - sim.used < TEXT_FRAMES);
        pool.end_frame();
        let now = pool.frame;
        rasters.retain(|_, (.., used)| now - *used < TEXT_FRAMES);
        counted.compiled =
            self.pipes().created.load(std::sync::atomic::Ordering::Relaxed).wrapping_sub(created.1);
        (counted.created, counted.reused, counted.textures) =
            (pool.created - created.0 + counted.compiled, reused, pool.textures() as u32);
        if let Some(s) = swarms {
            (counted.created, counted.steps) = (counted.created + s.created, s.steps);
            (counted.particles, counted.sim_us) = (s.slots(), s.micros);
        }
        *stats = counted;
        Ok(&target.as_ref().expect("created above").1)
    }
}
