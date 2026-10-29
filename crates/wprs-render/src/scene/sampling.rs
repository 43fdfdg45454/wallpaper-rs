use super::item::{invert, scale_of};
use super::{Image, SceneAssets};
use crate::Renderer;
use crate::texture::gpu_image;
use std::collections::HashMap;
use wgpu::TextureUsages as U;
use wprs_scene::{Affine, Content, Filter, Frame, Scene, Wrapping};

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Sampling {
    pub(crate) nearest: bool,
    pub(crate) wrap: Wrapping,
    pub(crate) mipmaps: Option<bool>,
    pub(crate) cells: Vec<[f32; 4]>,
}

fn grid(rows: u32, columns: u32, count: u32) -> Vec<[f32; 4]> {
    let (r, c) = (rows.max(1) as f32, columns.max(1) as f32);
    (0..count)
        .map(|i| [(i % columns.max(1)) as f32 / c, (i / columns.max(1)) as f32 / r, 1.0 / c, 1.0 / r])
        .collect()
}

pub(crate) fn layout(frames: usize) -> (u32, u32) {
    let columns = (frames as f32).sqrt().ceil().max(1.0) as u32;
    (frames.div_ceil(columns as usize) as u32, columns)
}

pub(crate) fn samplings(scene: &Scene, assets: &SceneAssets) -> HashMap<String, Sampling> {
    let one = |(id, a): (&String, &wprs_scene::Asset)| {
        let size = assets.images.get(id).map_or([1.0; 2], |i| [i.width() as f32, i.height() as f32]);
        let cells = match (&a.rects[..], a.atlas, assets.sheets.get(id).filter(|_| a.animated)) {
            (_, _, Some(d)) => {
                let (rows, columns) = layout(d.len());
                grid(rows, columns, d.len() as u32)
            }
            ([], Some([rows, columns]), _) => grid(rows, columns, rows.saturating_mul(columns).min(256)),
            ([], None, _) => Vec::new(),
            (rects, ..) => rects.iter().map(|r| [0, 1, 2, 3].map(|i| r[i] as f32 / size[i % 2])).collect(),
        };
        let nearest = a.filter == Some(Filter::Nearest);
        let sampling = Sampling { nearest, wrap: a.wrap.unwrap_or_default(), mipmaps: a.mipmaps, cells };
        (sampling != Sampling::default()).then(|| (id.clone(), sampling))
    };
    scene.assets.iter().filter_map(one).collect()
}

impl Renderer {
    pub(crate) fn mipped(&self, image: &Image) -> Image {
        let (source, natural) = image;
        let (w, h) = source.size();
        let levels = 32 - w.max(h).leading_zeros();
        if levels <= 1 || source.texture.mip_level_count() > 1 {
            return image.clone();
        }
        let format = source.texture.format();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wprs-scene mipmaps"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: U::TEXTURE_BINDING | U::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let pipes = self.pipes();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for level in 0..levels {
            let from = match level {
                0 => source.view.clone(),
                _ => texture.create_view(&super::mips::level(level - 1)),
            };
            let to = texture.create_view(&super::mips::level(level));
            pipes.shrink(&self.device, &mut encoder, &from, &to, format);
        }
        self.queue.submit([encoder.finish()]);
        (gpu_image(texture), *natural)
    }

    pub(crate) fn mipmap(&self, images: &mut HashMap<String, Image>, sampling: &HashMap<String, Sampling>) {
        for id in sampling.iter().filter(|(_, s)| s.mipmaps == Some(true)).map(|(id, _)| id) {
            if let Some(image) = images.get(id) {
                let mipped = self.mipped(image);
                images.insert(id.clone(), mipped);
            }
        }
    }

    pub(crate) fn shrunk(&self, view: &mut super::SceneView, frame: &Frame<'_>, screen: Affine) {
        for d in &frame.draws {
            let Content::Image { asset, size, frame: cell } = &d.content else { continue };
            let s = view.samplings.get(asset.as_ref());
            let Some((image, natural)) = view.images.get(asset.as_ref()) else { continue };
            if s.is_some_and(|s| s.mipmaps.is_some()) || image.texture.mip_level_count() > 1 {
                continue;
            }
            let rect = cell.and_then(|c| s?.cells.get(c as usize).copied()).unwrap_or([0.0, 0.0, 1.0, 1.0]);
            let shown = size.unwrap_or([natural[0] * rect[2], natural[1] * rect[3]]);
            let texels = (image.size().0 as f32 * rect[2]) * (image.size().1 as f32 * rect[3]);
            let pixels = scale_of(screen.then(d.world)).powi(2) * shown[0] * shown[1];
            if pixels < 0.5625 * texels && invert(d.world).0 != [0.0; 6] {
                let mipped = self.mipped(&(image.clone(), *natural));
                view.images.insert(asset.to_string(), mipped);
            }
        }
    }
}
