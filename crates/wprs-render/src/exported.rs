use crate::bind::Slot;
use crate::dmabuf::Dmabufs;
use crate::draw::TARGET_FORMAT;
use crate::{Layer, RenderError, Renderer};
use ash::vk::{self, ExternalMemoryHandleTypeFlags as Handle};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use wgpu::hal::api::Vulkan;
use wgpu::wgt::TextureUses;
use wprs_core::{Ages, Region};

pub struct Exported {
    texture: wgpu::Texture,
    view: Option<wgpu::TextureView>,
    raw: vk::Image,
    inner: Option<Arc<Copied>>,
    slots: Mutex<Vec<Slot>>,
    drawn: AtomicU64,
}

pub(crate) struct Copied {
    size: (u32, u32),
    drawn: Mutex<(wgpu::Texture, wgpu::TextureView, u64)>,
}

pub(crate) type Copies = Arc<Mutex<Vec<Weak<Copied>>>>;

pub(crate) fn shared(device: &wgpu::Device, copies: &Copies, size: (u32, u32)) -> Arc<Copied> {
    let mut kept = copies.lock().unwrap_or_else(PoisonError::into_inner);
    kept.retain(|c| c.strong_count() > 0);
    if let Some(found) = kept.iter().filter_map(Weak::upgrade).find(|c| c.size == size) {
        return found;
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wprs-render dmabuf copy"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let made = Arc::new(Copied { size, drawn: Mutex::new((texture, view, 0)) });
    kept.push(Arc::downgrade(&made));
    made
}

impl Exported {
    pub(crate) unsafe fn adopt(
        device: &wgpu::Device,
        hal: &wgpu::hal::vulkan::Device,
        (raw, memory): (vk::Image, vk::DeviceMemory),
        (width, height): (u32, u32),
        inner: Option<Arc<Copied>>,
    ) -> Self {
        let copy = inner.is_some();
        let (label, size) =
            (Some("wprs-render dmabuf"), wgpu::Extent3d { width, height, depth_or_array_layers: 1 });
        let (dimension, format) = (wgpu::TextureDimension::D2, TARGET_FORMAT);
        let usage = if copy { TextureUses::COPY_DST } else { TextureUses::COLOR_TARGET };
        let memory_flags = wgpu::hal::MemoryFlags::empty();
        let described = wgpu::hal::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            memory_flags,
            view_formats: Vec::new(),
        };
        let dedicated = wgpu::hal::vulkan::TextureMemory::Dedicated(memory);
        let texture = unsafe { hal.texture_from_raw(raw, &described, None, dedicated) };
        let usage = if copy { wgpu::TextureUsages::COPY_DST } else { wgpu::TextureUsages::RENDER_ATTACHMENT };
        let desc = wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            view_formats: &[],
        };
        let texture =
            unsafe { device.create_texture_from_hal::<Vulkan>(texture, &desc, TextureUses::UNINITIALIZED) };
        let view = (!copy).then(|| texture.create_view(&Default::default()));
        Self { view, texture, raw, inner, slots: Mutex::default(), drawn: AtomicU64::new(0) }
    }
}

impl Renderer {
    pub fn dmabufs(&self, emulate: bool) -> Option<Dmabufs> {
        Dmabufs::new(&self.device, if emulate { Handle::OPAQUE_FD } else { Handle::DMA_BUF_EXT })
    }

    pub fn render_exported(
        &self,
        image: &Exported,
        layers: &[Layer<'_>],
        ages: &Ages,
    ) -> Result<(), RenderError> {
        let mut slots = image.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let mut inner = image.inner.as_ref().map(|c| c.drawn.lock().unwrap_or_else(PoisonError::into_inner));
        let size = (image.texture.width(), image.texture.height());
        let fresh = |serial: u64| ages.since(serial).map(|r| r.within(size)).filter(|r| !r.covers(size));
        let (mine, kept) = (fresh(image.drawn.load(Relaxed)), image.drawn.swap(0, Relaxed) != 0);
        let done = self.scoped(|| {
            let (view, clip) = match inner.as_deref() {
                Some((_, view, at)) => (view, fresh(*at)),
                None => (image.view.as_ref().expect("a view to draw into"), mine),
            };
            let mut draw = self.encode(view, &mut slots, layers, size, TARGET_FORMAT, clip);
            if let Some((texture, ..)) = inner.as_deref() {
                let r = mine.unwrap_or(Region::whole(size));
                let origin = wgpu::Origin3d { x: r.x, y: r.y, z: 0 };
                let (from, to) = (texture.as_image_copy(), image.texture.as_image_copy());
                let (from, to) = (
                    wgpu::TexelCopyTextureInfo { origin, ..from },
                    wgpu::TexelCopyTextureInfo { origin, ..to },
                );
                let extent = wgpu::Extent3d { width: r.width, height: r.height, depth_or_array_layers: 1 };
                if !r.is_empty() {
                    draw.copy_texture_to_texture(from, to, extent);
                }
            }
            let copy = image.inner.is_some();
            let [acquire, release] =
                [false, true].map(|release| self.barrier(image.raw, release, copy, kept));
            self.queue.submit([acquire, draw.finish(), release])
        });
        let done = done.inspect_err(|_| slots.clear())?;
        let wait = wgpu::PollType::Wait { submission_index: Some(done), timeout: Some(self.gpu_timeout()) };
        self.polled(wait)?;
        image.drawn.store(ages.serial(), Relaxed);
        if let Some(inner) = inner.as_mut() {
            inner.2 = ages.serial();
        }
        Ok(())
    }
}

#[test]
fn buffers_of_one_size_share_one_copy_texture() {
    let r = match Renderer::shared() {
        Ok(r) => r,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let copies = Copies::default();
    let [a, b] = [(); 2].map(|()| shared(&r.device, &copies, (64, 32)));
    let other = shared(&r.device, &copies, (32, 32));
    assert!(Arc::ptr_eq(&a, &b) && !Arc::ptr_eq(&a, &other));
    drop((a, b, other));
    let again = shared(&r.device, &copies, (64, 32));
    assert_eq!(copies.lock().unwrap().len(), 1, "a copy texture outlived its buffers");
    assert_eq!(again.size, (64, 32));
}
