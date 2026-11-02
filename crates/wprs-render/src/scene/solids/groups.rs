use super::uniform::{DrawSlots, Finish3, Object, Scene3};
use super::{GpuModel, Staged};
use crate::Renderer;
use crate::texture::GpuImage;
use wgpu::BindingResource as R;

fn bound(buffer: &wgpu::Buffer, size: usize) -> R<'_> {
    R::Buffer(wgpu::BufferBinding { buffer, offset: 0, size: wgpu::BufferSize::new(size as u64) })
}

fn entry(binding: u32, resource: R<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

impl Renderer {
    fn solid_group(&self, layout: usize, entries: &[wgpu::BindGroupEntry]) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render solid"),
            layout: &self.solids().layouts[layout],
            entries,
        })
    }

    pub(crate) fn stage_groups(&self, s: &mut Staged, env: &GpuImage, frame: &wgpu::Buffer) {
        if s.groups.as_ref().is_some_and(|(id, _)| *id == env.id) {
            return;
        }
        let solids = self.solids();
        let scene = |casting: bool| {
            let map = |k: usize| match casting {
                true => &solids.no_shadow,
                false => s.maps[k].as_ref().map_or(&solids.no_shadow, |m| &m.view),
            };
            self.solid_group(
                0,
                &[
                    entry(0, bound(&s.scene, size_of::<Scene3>())),
                    entry(1, R::TextureView(&env.view)),
                    entry(2, R::Sampler(&solids.wrap)),
                    entry(3, frame.as_entire_binding()),
                    entry(4, R::TextureView(map(0))),
                    entry(5, R::TextureView(map(1))),
                    entry(6, R::Sampler(&solids.compare)),
                    entry(7, s.shades.as_entire_binding()),
                ],
            )
        };
        let (lit, casting) = (scene(false), scene(true));
        let finishes = self.solid_group(3, &[entry(0, bound(&s.finishes, size_of::<Finish3>()))]);
        s.groups = Some((env.id, [lit, finishes, casting]));
    }

    pub(crate) fn object_group(&self, s: &mut Staged, digest: u64, gpu: &GpuModel) {
        if s.objects_groups.contains_key(&digest) {
            return;
        }
        let solids = self.solids();
        let morphs = gpu.morphs.as_ref().unwrap_or(&solids.no_morph);
        let group = self.solid_group(
            1,
            &[
                entry(0, bound(&s.objects, size_of::<Object>())),
                entry(1, bound(&s.bones, super::BONES_SLOT as usize)),
                entry(2, bound(&s.draws, size_of::<DrawSlots>())),
                entry(3, R::TextureView(morphs)),
            ],
        );
        s.objects_groups.insert(digest, group);
    }
}
