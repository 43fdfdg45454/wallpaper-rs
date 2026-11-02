use super::{DEPTH, SCENE_SLOT, SLOT};
use crate::Renderer;
use crate::texture::{GpuImage, create_texture, gpu_image};
use wgpu::BufferUsages as B;
use wgpu::TextureUsages as U;
use wprs_scene::{MAX_FIGURES, MAX_PLANES};

pub(crate) struct Staged {
    pub(crate) key: u64,
    pub(crate) size: (u32, u32),
    pub(crate) hdr: bool,
    pub(crate) slices: Vec<GpuImage>,
    pub(crate) depth: wgpu::TextureView,
    pub(crate) scene: wgpu::Buffer,
    pub(crate) objects: wgpu::Buffer,
    pub(crate) finishes: wgpu::Buffer,
    pub(crate) room: usize,
    pub(crate) groups: Option<(u64, [wgpu::BindGroup; 3])>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) renders: u64,
}

fn bound(buffer: &wgpu::Buffer, size: u64) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer,
        offset: 0,
        size: wgpu::BufferSize::new(size),
    })
}

impl Renderer {
    fn uniform(&self, label: &str, size: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: B::UNIFORM | B::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub(crate) fn staged(&self, size: (u32, u32), slices: usize, hdr: bool, room: usize) -> Staged {
        let format = self.solid_format(hdr);
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC;
        let image = || gpu_image(create_texture(&self.device, "wprs-render solid", size, format, usage));
        let depth =
            create_texture(&self.device, "wprs-render solid depth", size, DEPTH, U::RENDER_ATTACHMENT);
        Staged {
            key: 0,
            size,
            hdr,
            slices: (0..slices).map(|_| image()).collect(),
            depth: depth.create_view(&Default::default()),
            scene: self.uniform("wprs-render solid scene", SCENE_SLOT * (MAX_PLANES as u64 + 1)),
            objects: self.uniform("wprs-render solid objects", SLOT * MAX_FIGURES as u64),
            finishes: self.uniform("wprs-render solid finishes", SLOT * room.max(1) as u64),
            room: room.max(1),
            groups: None,
            bytes: Vec::new(),
            renders: 0,
        }
    }

    pub(crate) fn stage_groups(&self, s: &mut Staged, env: &GpuImage) {
        if s.groups.as_ref().is_some_and(|(id, _)| *id == env.id) {
            return;
        }
        let solids = self.solids();
        let group = |layout: usize, entries: &[wgpu::BindGroupEntry]| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("wprs-render solid"),
                layout: &solids.layouts[layout],
                entries,
            })
        };
        let scene = group(
            0,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: bound(&s.scene, std::mem::size_of::<super::uniform::Scene3>() as u64),
                },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&env.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&solids.wrap) },
            ],
        );
        let object = std::mem::size_of::<super::uniform::Object>() as u64;
        let objects = group(1, &[wgpu::BindGroupEntry { binding: 0, resource: bound(&s.objects, object) }]);
        let finish = std::mem::size_of::<super::uniform::Finish3>() as u64;
        let finishes = group(3, &[wgpu::BindGroupEntry { binding: 0, resource: bound(&s.finishes, finish) }]);
        s.groups = Some((env.id, [scene, objects, finishes]));
    }
}
