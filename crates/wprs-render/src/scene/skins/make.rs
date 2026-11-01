use super::{BONES, DEPTH, FORMAT, PIECES, Skinner};
use crate::texture::{GpuImage, create_texture, gpu_image};
use crate::{RenderError, Renderer};
use wgpu::{BufferUsages as B, TextureUsages as U};
use wprs_scene::{MORPH_WIDTH, Rigged};

pub(crate) struct Skin {
    pub(crate) digest: u64,
    pub(crate) generation: u64,
    pub(crate) relief: Option<wprs_scene::Relief>,
    pub(crate) image: GpuImage,
    pub(crate) depth: wgpu::TextureView,
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    pub(crate) bones: wgpu::Buffer,
    pub(crate) pieces: wgpu::Buffer,
    pub(crate) morphs: wgpu::TextureView,
    pub(crate) own: Option<GpuImage>,
    pub(crate) group: Option<(u64, wgpu::BindGroup)>,
    pub(crate) scratch: Vec<[f32; 4]>,
}

fn embedded(r: &Renderer, rig: &Rigged) -> Option<GpuImage> {
    let reader =
        image::ImageReader::new(std::io::Cursor::new(rig.picture.as_deref()?)).with_guessed_format().ok()?;
    let rgba = crate::decode_limited(reader, crate::picture_limits()).ok()?.into_rgba8();
    r.upload_rgba(rgba.as_raw(), rgba.width(), rgba.height(), rgba.width() * 4).ok()
}

impl Renderer {
    pub(crate) fn skinner(&self) -> &Skinner {
        self.skinner.get_or_init(|| super::skinner(&self.device))
    }

    pub(crate) fn skin_targets(&self, size: (u32, u32)) -> (GpuImage, wgpu::TextureView) {
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC;
        let image = gpu_image(create_texture(&self.device, "wprs-render skin", size, FORMAT, usage));
        let depth = create_texture(&self.device, "wprs-render skin depth", size, DEPTH, U::RENDER_ATTACHMENT);
        (image, depth.create_view(&Default::default()))
    }

    pub(crate) fn fresh_skin(&self, rig: &Rigged, size: (u32, u32)) -> Result<Skin, RenderError> {
        self.scoped(|| {
            let (image, depth) = self.skin_targets(size);
            let fill = |label, bytes: &[u8], usage| {
                crate::filled::filled((&self.device, &self.queue), label, bytes, usage)
            };
            let vertices = fill("wprs-render skin vertices", bytemuck::cast_slice(&rig.vertices), B::VERTEX);
            let indices = fill("wprs-render skin indices", bytemuck::cast_slice(&rig.indices), B::INDEX);
            let zeros = |n: usize| vec![0u8; n * 16];
            let bones = fill("wprs-render skin bones", &zeros(BONES), B::UNIFORM);
            let pieces = fill("wprs-render skin pieces", &zeros(PIECES), B::UNIFORM);
            let texels = rig.morphs.len().max(1);
            let size = ((texels.min(MORPH_WIDTH)) as u32, texels.div_ceil(MORPH_WIDTH) as u32);
            let morphs = create_texture(
                &self.device,
                "wprs-render skin shapes",
                size,
                wgpu::TextureFormat::Rgba32Float,
                U::TEXTURE_BINDING | U::COPY_DST,
            );
            let mut data: Vec<[f32; 4]> = rig.morphs.clone();
            data.resize((size.0 * size.1) as usize, [0.0; 4]);
            self.queue.write_texture(
                morphs.as_image_copy(),
                bytemuck::cast_slice(&data),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size.0 * 16),
                    rows_per_image: None,
                },
                wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            );
            let view = |t: &wgpu::Texture| t.create_view(&Default::default());
            Skin {
                digest: rig.digest,
                generation: 0,
                relief: None,
                image,
                depth,
                vertices,
                indices,
                bones,
                pieces,
                morphs: view(&morphs),
                own: embedded(self, rig),
                group: None,
                scratch: Vec::with_capacity(PIECES),
            }
        })
    }

    pub(crate) fn bind_skin(&self, skin: &mut Skin, (id, picture): (u64, &wgpu::TextureView)) {
        if skin.group.as_ref().is_some_and(|(found, _)| *found == id) {
            return;
        }
        let entries = [
            wgpu::BindGroupEntry { binding: 0, resource: skin.bones.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: skin.pieces.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&skin.morphs) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(picture) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.clamp) },
        ];
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render skin"),
            layout: &self.skinner().layout,
            entries: &entries,
        });
        skin.group = Some((id, group));
    }
}
