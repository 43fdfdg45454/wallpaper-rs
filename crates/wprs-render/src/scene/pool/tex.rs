use crate::texture::{create_texture, next_id};
use wgpu::{TextureFormat, TextureUsages as U};

#[derive(Clone)]
pub(crate) struct Tex {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) id: u64,
}

impl Tex {
    pub(crate) fn new(device: &wgpu::Device, (w, h): (u32, u32), format: TextureFormat) -> Tex {
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC | U::COPY_DST;
        let texture = create_texture(device, "wprs-scene texture", (w.max(1), h.max(1)), format, usage);
        Tex { view: texture.create_view(&Default::default()), texture, id: next_id() }
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }

    pub(crate) fn bytes(&self) -> u64 {
        bytes(self.size(), self.texture.format())
    }
}

pub(crate) fn bytes((w, h): (u32, u32), format: TextureFormat) -> u64 {
    u64::from(w) * u64::from(h) * u64::from(format.block_copy_size(None).unwrap_or(8))
}

impl super::super::item::Rect {
    pub(crate) fn bucket(self) -> Self {
        let up = |v: f32| {
            let step = (v.max(1.0).log2().floor() - 3.0).exp2().max(64.0);
            (v / step).ceil() * step
        };
        Self { w: up(self.w), h: up(self.h), ..self }
    }
}
