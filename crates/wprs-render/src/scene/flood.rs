use super::field::steps;
use crate::Renderer;
use crate::texture::{GpuImage, create_texture, gpu_image};
use image::RgbaImage;
use wgpu::{TextureFormat, TextureUsages};

const FAR: f32 = 30000.0;
const SIDES: [[i32; 2]; 4] = [[1, 0], [-1, 0], [0, 1], [0, -1]];

struct Cover<'a> {
    image: &'a RgbaImage,
    w: i32,
    h: i32,
}

impl Cover<'_> {
    fn alpha(&self, x: i32, y: i32) -> f32 {
        let (x, y) = (x.clamp(0, self.w - 1) as u32, y.clamp(0, self.h - 1) as u32);
        f32::from(self.image.get_pixel(x, y)[3]) / 255.0
    }

    fn seed(&self, x: i32, y: i32) -> [f32; 2] {
        let c = self.alpha(x, y);
        let inside = c >= 0.5;
        let near = SIDES.map(|[dx, dy]| self.alpha(x + dx, y + dy));
        let edge = (c > 0.0 && c < 1.0) || near.iter().any(|a| (*a >= 0.5) != inside);
        if !edge {
            return [FAR; 2];
        }
        let g = [(near[0] - near[1]) * 0.5, (near[2] - near[3]) * 0.5];
        let n = g[0] * g[0] + g[1] * g[1];
        match n > 1e-6 {
            true => g.map(|v| (v * (0.5 - c) / n).clamp(-1.0, 1.0)),
            false => [0.0; 2],
        }
    }
}

pub(crate) fn flood(image: &RgbaImage, reach: u32) -> Vec<[f32; 2]> {
    let cover = Cover { image, w: image.width() as i32, h: image.height() as i32 };
    let (w, h) = (cover.w, cover.h);
    let mut seeds: Vec<[f32; 2]> =
        (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| cover.seed(x, y)).collect();
    for k in steps(reach).into_iter().map(|s| s as i32) {
        let jumped = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| {
            let (mut best, mut near) = ([FAR; 2], 1e20);
            for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
                let (qx, qy) = (x + dx * k, y + dy * k);
                if qx < 0 || qy < 0 || qx >= w || qy >= h {
                    continue;
                }
                let off = seeds[(qy * w + qx) as usize];
                let to = [(qx - x) as f32 + off[0], (qy - y) as f32 + off[1]];
                let d = to[0] * to[0] + to[1] * to[1];
                if off[0] < FAR * 0.5 && d < near {
                    (best, near) = (to, d);
                }
            }
            best
        });
        seeds = jumped.collect();
    }
    let field = |(i, to): (usize, &[f32; 2])| {
        let c = cover.alpha(i as i32 % w, i as i32 / w);
        let d = if to[0] >= FAR * 0.5 { FAR } else { to[0].hypot(to[1]) };
        [c, if c >= 0.5 { -d } else { d }]
    };
    seeds.iter().enumerate().map(field).collect()
}

impl Renderer {
    pub(crate) fn floods(&self) -> bool {
        let half = self.gpu.get_texture_format_features(TextureFormat::Rgba16Float);
        half.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
            && half.allowed_usages.contains(TextureUsages::TEXTURE_BINDING)
    }

    pub(crate) fn flooded(&self, image: &RgbaImage, reach: u32) -> GpuImage {
        let half = |v: f32| half::f16::from_f32(v).to_bits();
        let texels: Vec<u16> =
            flood(image, reach).into_iter().flat_map(|[c, d]| [c, d, 0.0, 1.0]).map(half).collect();
        let size = image.dimensions();
        let usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        let texture =
            create_texture(&self.device, "wprs-scene text field", size, TextureFormat::Rgba16Float, usage);
        let layout =
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size.0 * 8), rows_per_image: None };
        let extent = wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 };
        self.queue.write_texture(texture.as_image_copy(), bytemuck::cast_slice(&texels), layout, extent);
        gpu_image(texture)
    }
}

#[cfg(test)]
mod tests;
