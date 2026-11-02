use super::{BONES_SLOT, DEPTH, FINISH_SLOT, SCENE_SLOT, SLOT};
use crate::Renderer;
use crate::texture::{GpuImage, create_texture, gpu_image};
use std::collections::HashMap;
use wgpu::BufferUsages as B;
use wgpu::TextureUsages as U;
use wprs_scene::{MAX_FIGURES, MAX_PLANES};

pub(crate) const SCENE_SLOTS: u64 = 2 * (MAX_PLANES as u64 + 1) + 12;

pub(crate) struct ShadowMap {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) side: u32,
    pub(crate) layers: u32,
    pub(crate) key: u64,
    pub(crate) renders: u64,
}

pub(crate) struct Staged {
    pub(crate) key: u64,
    pub(crate) size: (u32, u32),
    pub(crate) hdr: bool,
    pub(crate) slices: Vec<GpuImage>,
    pub(crate) cross: Vec<GpuImage>,
    pub(crate) depth: wgpu::TextureView,
    pub(crate) scene: wgpu::Buffer,
    pub(crate) objects: wgpu::Buffer,
    pub(crate) finishes: wgpu::Buffer,
    pub(crate) bones: wgpu::Buffer,
    pub(crate) draws: wgpu::Buffer,
    pub(crate) shades: wgpu::Buffer,
    pub(crate) room: usize,
    pub(crate) draw_room: usize,
    pub(crate) bone_room: usize,
    pub(crate) maps: [Option<ShadowMap>; 2],
    pub(crate) groups: Option<(u64, [wgpu::BindGroup; 3])>,
    pub(crate) cross_groups: Vec<wgpu::BindGroup>,
    pub(crate) objects_groups: HashMap<u64, wgpu::BindGroup>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) calls: Vec<super::calls::Call>,
    pub(crate) objects_bytes: Vec<u8>,
    pub(crate) finish_bytes: Vec<u8>,
    pub(crate) bone_bytes: Vec<u8>,
    pub(crate) draw_bytes: Vec<u8>,
    pub(crate) renders: u64,
    pub(crate) shadow_renders: u64,
}

impl Renderer {
    pub(super) fn uniform(&self, label: &str, size: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: B::UNIFORM | B::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub(super) fn slice_images(&self, size: (u32, u32), hdr: bool, count: usize) -> Vec<GpuImage> {
        let format = self.solid_format(hdr);
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC;
        (0..count)
            .map(|_| gpu_image(create_texture(&self.device, "wprs-render solid", size, format, usage)))
            .collect()
    }

    pub(crate) fn staged(
        &self,
        size: (u32, u32),
        slices: usize,
        hdr: bool,
        room: (usize, usize, usize),
    ) -> Staged {
        let depth =
            create_texture(&self.device, "wprs-render solid depth", size, DEPTH, U::RENDER_ATTACHMENT);
        let shades = std::mem::size_of::<super::uniform::Shadows>() as u64;
        let (room, draw_room, bone_room) = (room.0.max(1), room.1.max(1), room.2.max(1));
        Staged {
            key: 0,
            size,
            hdr,
            slices: self.slice_images(size, hdr, slices),
            cross: Vec::new(),
            depth: depth.create_view(&Default::default()),
            scene: self.uniform("wprs-render solid scene", SCENE_SLOT * SCENE_SLOTS),
            objects: self.uniform("wprs-render solid objects", SLOT * MAX_FIGURES as u64),
            finishes: self.uniform("wprs-render solid finishes", FINISH_SLOT * room as u64),
            bones: self.uniform("wprs-render solid bones", BONES_SLOT * bone_room as u64),
            draws: self.uniform("wprs-render solid draws", SLOT * draw_room as u64),
            shades: self.uniform("wprs-render solid shadows", shades),
            room,
            draw_room,
            bone_room,
            maps: [None, None],
            groups: None,
            cross_groups: Vec::new(),
            objects_groups: HashMap::new(),
            bytes: Vec::new(),
            calls: Vec::new(),
            objects_bytes: Vec::new(),
            finish_bytes: Vec::new(),
            bone_bytes: Vec::new(),
            draw_bytes: Vec::new(),
            renders: 0,
            shadow_renders: 0,
        }
    }
}
