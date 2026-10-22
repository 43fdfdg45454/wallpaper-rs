mod draw;
mod effects;
mod encode;
mod item;
mod paint;
mod pipes;
mod pool;
mod text;

use crate::texture::{GpuImage, create_image, gpu_image};
use crate::{RenderError, Renderer};
use image::RgbaImage;
use item::{FrameData, Rect, frame_data, linear};
use paint::Painter;
use pipes::Mode;
pub(crate) use pipes::Pipes;
use pool::{Pool, Tex};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{TextureFormat as Format, TextureUsages as Usage};
use wprs_core::{Fit, UvRect};
use wprs_scene::{Affine, BlendSpace, Frame, Quality, Runtime, Scene};

const TEXT_FRAMES: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Src {
    Pool(usize),
    Ext(usize),
    Cache(usize),
    White,
    Clear,
    Canvas,
}

pub(crate) enum Op {
    Clear(Src, [f64; 4]),
    Copy { from: Src, to: Src, origin: [u32; 2], size: (u32, u32), at: [u32; 2] },
    Draw { target: Src, entry: &'static str, mode: Mode, slot: u32, tex: [Src; 3] },
    Effect { target: Src, entry: &'static str, slot: u32, tex: [Src; 6] },
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
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SceneStats {
    pub passes: u32,
    pub draws: u32,
    pub created: u32,
    pub textures: u32,
    pub reused: u32,
}

pub struct SceneView {
    fit: Fit,
    canvas: [u32; 2],
    srgb: bool,
    format: Format,
    images: HashMap<String, Image>,
    fonts: text::Fonts,
    texts: HashMap<u64, (GpuImage, [f32; 2], u64)>,
    pool: Pool,
    caches: HashMap<usize, Cached>,
    target: Option<(Tex, GpuImage)>,
    uniform: wgpu::Buffer,
    stats: SceneStats,
}

impl SceneView {
    pub fn view(&self, size: (u32, u32)) -> [f32; 4] {
        let fit = match self.fit {
            Fit::Tile => Fit::Center,
            Fit::Span => Fit::Fill,
            fit => fit,
        };
        let uv = UvRect::compute(fit, (self.canvas[0], self.canvas[1]), size);
        let [cw, ch] = self.canvas.map(|v| v as f32);
        [uv.offset[0] * cw, uv.offset[1] * ch, uv.scale[0] * cw, uv.scale[1] * ch]
    }

    pub fn stats(&self) -> SceneStats {
        self.stats
    }

    pub fn trim(&mut self) {
        self.pool.trim();
    }

    pub fn image(&self) -> Option<&GpuImage> {
        self.target.as_ref().map(|(_, image)| image)
    }

    fn screen(&self, size: (u32, u32)) -> Affine {
        let [x, y, w, h] = self.view(size);
        let (sx, sy) = (size.0 as f32 / w, size.1 as f32 / h);
        Affine([sx, 0.0, 0.0, sy, -x * sx, -y * sy])
    }
}

impl Renderer {
    fn pipes(&self) -> &Pipes {
        self.scene.get_or_init(|| Pipes::new(&self.device))
    }

    pub fn software(&self) -> bool {
        self.gpu.get_info().device_type == wgpu::DeviceType::Cpu
    }

    pub fn discrete(&self) -> bool {
        self.gpu.get_info().device_type == wgpu::DeviceType::DiscreteGpu
    }

    pub fn scene_view(&self, scene: &Scene, assets: &SceneAssets) -> Result<SceneView, RenderError> {
        let srgb = scene.canvas.blend_space == BlendSpace::Srgb;
        let half = self.gpu.get_texture_format_features(Format::Rgba16Float);
        let flags = wgpu::TextureFormatFeatureFlags::FILTERABLE | wgpu::TextureFormatFeatureFlags::BLENDABLE;
        let usable = half.allowed_usages.contains(Usage::RENDER_ATTACHMENT) && half.flags.contains(flags);
        let format = match (srgb, usable) {
            (true, _) => Format::Rgba8Unorm,
            (false, true) => Format::Rgba16Float,
            (false, false) => Format::Rgba8UnormSrgb,
        };
        self.scoped(|| {
            let _ = self.pipes();
            let mut shared: HashMap<*const RgbaImage, Image> = HashMap::new();
            let upload = |(id, image): (&String, &Arc<RgbaImage>)| {
                let image = shared.entry(Arc::as_ptr(image)).or_insert_with(|| {
                    let prepared = crate::decode::scaled(image, 1.0, self.max_edge());
                    let (w, h) = prepared.dimensions();
                    let gpu = create_image(&self.device, &self.queue, prepared.as_raw(), w, h, w * 4);
                    (gpu, [image.width() as f32, image.height() as f32])
                });
                (id.clone(), image.clone())
            };
            let usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
            let size = std::mem::size_of::<FrameData>() as u64;
            SceneView {
                fit: scene.canvas.fit,
                canvas: scene.canvas.size,
                srgb,
                format,
                images: assets.images.iter().map(upload).collect(),
                fonts: text::Fonts::new(&assets.fonts),
                texts: HashMap::new(),
                pool: Pool::new(&self.device, &self.queue, format),
                caches: HashMap::new(),
                target: None,
                uniform: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("wprs-scene frame"),
                    size,
                    usage,
                    mapped_at_creation: false,
                }),
                stats: SceneStats::default(),
            }
        })
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
        let created = view.pool.created;
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
        let SceneView { pool, caches, images, texts: rasters, target, uniform, srgb, format, stats, .. } =
            view;
        let rasters_of =
            |(i, key): (usize, u64)| rasters.get(&key).map(|(image, size, _)| (i, (image, *size)));
        let texts: HashMap<usize, (&GpuImage, [f32; 2])> = texts.into_iter().filter_map(rasters_of).collect();
        let full = Rect { x: 0.0, y: 0.0, w: size.0 as f32, h: size.1 as f32 };
        let (srgb, format) = (*srgb, *format);
        let mut painter = Painter {
            r: self,
            frame,
            pool,
            caches,
            images,
            texts: &texts,
            videos,
            exts: Vec::new(),
            screen,
            full,
            srgb,
            format,
            ops: Vec::new(),
            reused: 0,
            kept: Vec::new(),
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
        let (canvas, output) = target.as_ref().expect("created above");
        let sources = encode::Sources { caches, exts: &exts, canvas, output, format };
        let mut counted = self.scoped(|| self.encode_scene(pool, &sources, &ops, resolve))?;
        pool.end_frame();
        let now = pool.frame;
        rasters.retain(|_, (.., used)| now - *used < TEXT_FRAMES);
        (counted.created, counted.reused, counted.textures) =
            (pool.created - created, reused, pool.textures() as u32);
        *stats = counted;
        Ok(&target.as_ref().expect("created above").1)
    }
}
