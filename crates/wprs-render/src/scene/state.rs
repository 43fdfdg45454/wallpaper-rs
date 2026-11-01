use super::Pixels;
use super::paint::Painter;
use super::pool::Tex;
use super::{Image, Src};
use crate::Renderer;
use crate::texture::next_id;
use image::{Rgba, RgbaImage};
use wprs_scene::{Color, EffectDraw, Param, ParamKind, Scene, Value};

pub(crate) struct Sim {
    pub(crate) tex: [Tex; 2],
    pub(crate) front: usize,
    pub(crate) used: u64,
    pub(crate) clock: Option<f32>,
    pub(crate) owed: f32,
    pub(crate) reset: u32,
}

pub(crate) fn volume(device: &wgpu::Device, queue: &wgpu::Queue, size: u32, data: &[[f32; 3]]) -> Tex {
    let half = |v: f32| half::f16::from_f32(v).to_bits();
    let texels: Vec<u16> = data.iter().flat_map(|&[r, g, b]| [r, g, b, 1.0]).map(half).collect();
    let extent = wgpu::Extent3d { width: size, height: size, depth_or_array_layers: size };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wprs-scene lut"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let layout =
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size * 8), rows_per_image: Some(size) };
    queue.write_texture(texture.as_image_copy(), bytemuck::cast_slice(&texels), layout, extent);
    Tex { view: texture.create_view(&Default::default()), texture, id: next_id() }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn asset(&mut self, e: &EffectDraw<'_>, param: &str, kind: ParamKind) -> Option<Src> {
        let def = e.descriptor.params.iter().find(|p| p.id == param && p.kind == kind)?;
        let bound = e.textures.iter().find(|(id, _)| *id == param).map(|(_, name)| name.as_str());
        let id = bound.or_else(|| def.named(e.effect));
        let (images, luts, paints) = (self.images, self.luts, self.paints);
        if kind == ParamKind::Lut {
            return luts.iter().position(|(name, _)| Some(name.as_str()) == id).map(Src::Lut);
        }
        let painted = |(owner, key, _): &&(String, String, Image)| match owner.is_empty() {
            true => Some(key.as_str()) == id,
            false => *owner == e.descriptor.id && *key == def.id,
        };
        let found = id.and_then(|id| images.get(id)).or_else(|| paints.iter().find(painted).map(|(.., i)| i));
        let moving = id.and_then(|id| self.moving(id));
        moving.or(found.map(|image| &image.0)).map(|image| self.ext(image))
    }

    pub(crate) fn lut(&mut self, e: &EffectDraw<'_>) -> Src {
        let found = e.descriptor.params.iter().find(|p| p.kind == ParamKind::Lut);
        found.and_then(|p| self.asset(e, &p.id, ParamKind::Lut)).unwrap_or(Src::Void)
    }
}

const GENERIC: [&str; 4] = ["white", "black", "normal_flat", "noise"];

fn generic(name: &str) -> RgbaImage {
    let mix = |v: u32| {
        let v = v.wrapping_mul(0x9e37_79b1);
        let v = (v ^ (v >> 15)).wrapping_mul(0x85eb_ca77);
        (v ^ (v >> 13)) as u8
    };
    match name {
        "black" => RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255])),
        "normal_flat" => RgbaImage::from_pixel(1, 1, Rgba([128, 128, 255, 255])),
        "noise" => RgbaImage::from_fn(64, 64, |x, y| {
            let k = (y * 64 + x) * 4;
            Rgba([mix(k), mix(k + 1), mix(k + 2), mix(k + 3)])
        }),
        _ => RgbaImage::from_pixel(1, 1, Rgba([255; 4])),
    }
}

fn painted(paint: &Param) -> RgbaImage {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let texel = match paint {
        Param::Color(Color(c)) => c.map(byte),
        Param::Number(n) => [byte(*n); 4],
        _ => [0; 4],
    };
    RgbaImage::from_pixel(1, 1, Rgba(texel))
}

impl Renderer {
    pub(crate) fn paints(&self, scene: &Scene) -> Vec<(String, String, Image)> {
        let mut out: Vec<(String, String, Image)> = Vec::new();
        for e in scene.drawable().flat_map(|l| &l.effects) {
            let Some(d) = scene.effect(&e.effect) else { continue };
            for p in d.params.iter().filter(|p| p.kind.asset() && p.kind != ParamKind::Lut) {
                let mapped = match e.params.get(&p.id) {
                    Some(Value::Bound(b)) => b.map.iter().flat_map(|m| m.values()).collect(),
                    _ => Vec::new(),
                };
                let names = p.named(e).into_iter().chain(mapped.into_iter().filter_map(|v| match v {
                    Param::Text(t) => Some(t.as_str()),
                    _ => None,
                }));
                let names = names.filter(|n| GENERIC.contains(n) && !scene.assets.contains_key(*n));
                let mut wanted: Vec<(String, String, RgbaImage)> =
                    names.map(|n| (String::new(), n.to_string(), generic(n))).collect();
                if let Some(paint) = p.paint.as_ref().filter(|_| p.kind == ParamKind::Mask) {
                    wanted.push((d.id.clone(), p.id.clone(), painted(paint)));
                }
                for (owner, key, pixels) in wanted {
                    if out.iter().any(|(o, k, _)| *o == owner && *k == key) {
                        continue;
                    }
                    if let Some(image) = self.upload_pixels(&Pixels::new(pixels), true, Some(false)) {
                        out.push((owner, key, image));
                    }
                }
            }
        }
        out
    }
}
