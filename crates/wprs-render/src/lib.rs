//! Headless wgpu renderer.
//!
//! Renders a wallpaper into an offscreen texture and reads it back as a
//! tightly packed BGRA8 frame, ready to be sent to a compositor or a
//! PipeWire stream.

use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use std::sync::mpsc;
use wprs_core::{Fit, UvRect};

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no compatible GPU adapter found: {0}")]
    NoAdapter(String),
    #[error("cannot create GPU device: {0}")]
    Device(String),
    #[error("invalid target size {0}x{1}")]
    InvalidSize(u32, u32),
    #[error("GPU readback failed: {0}")]
    Readback(String),
}

/// A rendered frame: BGRA8, sRGB, opaque, rows of `stride` bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("bytes", &self.data.len())
            .finish()
    }
}

impl Frame {
    /// Pixel at (x, y) as `[b, g, r, a]`.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y * self.stride + x * 4) as usize;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }
}

/// What to draw on one target with [`Renderer::render`].
#[derive(Debug, Clone, Copy)]
pub struct Scene<'a> {
    pub image: Option<&'a RgbaImage>,
    pub fit: Fit,
    /// sRGB background color.
    pub background: [u8; 3],
}

/// An image or video frame stored on the GPU.
pub struct GpuImage {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl GpuImage {
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// A reusable render target with its readback buffer.
pub struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    width: u32,
    height: u32,
}

impl Target {
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// One layer of a frame. Layers are drawn in order; each one covers the
/// whole target (image plus background) and is blended over the previous
/// layers with `opacity`.
#[derive(Clone, Copy)]
pub struct Layer<'a> {
    pub image: Option<&'a GpuImage>,
    pub fit: Fit,
    /// sRGB background color.
    pub background: [u8; 3],
    /// 0.0 (invisible) to 1.0 (opaque).
    pub opacity: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    offset: [f32; 2],
    scale: [f32; 2],
    background: [f32; 4],
    flags: [u32; 4],
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    opaque: wgpu::RenderPipeline,
    blended: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    clamp: wgpu::Sampler,
    repeat: wgpu::Sampler,
    placeholder: GpuImage,
    adapter: String,
    max_edge: u32,
}

impl Renderer {
    /// Creates a renderer on the best available adapter, software ones included.
    pub fn new() -> Result<Self, RenderError> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, RenderError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::GL);
        let instance = wgpu::Instance::new(desc);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let info = adapter.get_info();
        let adapter_limits = adapter.limits();
        let limits = wgpu::Limits {
            max_texture_dimension_2d: adapter_limits.max_texture_dimension_2d,
            ..wgpu::Limits::downlevel_defaults()
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("wprs-render"),
                required_limits: limits.clone(),
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError::Device(e.to_string()))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wprs-render shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wprs-render bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wprs-render pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("wprs-render pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: TARGET_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // Crossfade: result = layer * c + below * (1 - c), with c set per draw.
        let fade = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Constant,
            dst_factor: wgpu::BlendFactor::OneMinusConstant,
            operation: wgpu::BlendOperation::Add,
        };
        let opaque = pipeline(None);
        let blended = pipeline(Some(wgpu::BlendState {
            color: fade,
            alpha: fade,
        }));
        let sampler = |mode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("wprs-render sampler"),
                address_mode_u: mode,
                address_mode_v: mode,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            })
        };
        let clamp = sampler(wgpu::AddressMode::ClampToEdge);
        let repeat = sampler(wgpu::AddressMode::Repeat);
        let placeholder = create_image(&device, &queue, &[0u8; 4], 1, 1, 4);

        Ok(Self {
            adapter: format!("{} ({:?})", info.name, info.backend),
            max_edge: limits.max_texture_dimension_2d,
            device,
            queue,
            opaque,
            blended,
            layout,
            clamp,
            repeat,
            placeholder,
        })
    }

    /// Human readable adapter description, for diagnostics.
    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    /// Largest target edge supported by the device.
    pub fn max_edge(&self) -> u32 {
        self.max_edge
    }

    /// Uploads a still image prepared for showing on a `target`-sized output
    /// with `fit`: large images are downscaled on the CPU first, so the GPU
    /// never minifies by more than 2x and textures stay within device limits.
    pub fn upload(&self, image: &RgbaImage, fit: Fit, target: (u32, u32)) -> GpuImage {
        let prepared = prepare_image(image, fit, target, self.max_edge);
        let (w, h) = prepared.dimensions();
        create_image(&self.device, &self.queue, prepared.as_raw(), w, h, w * 4)
    }

    /// Uploads raw RGBA pixels with rows of `stride` bytes.
    pub fn upload_rgba(
        &self,
        rgba: &[u8],
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<GpuImage, RenderError> {
        check_pixels(rgba, width, height, stride, self.max_edge)?;
        Ok(create_image(
            &self.device,
            &self.queue,
            rgba,
            width,
            height,
            stride,
        ))
    }

    /// Replaces the pixels of `image`, reusing its texture when the size is
    /// unchanged (video frames).
    pub fn update_rgba(
        &self,
        image: &mut GpuImage,
        rgba: &[u8],
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<(), RenderError> {
        check_pixels(rgba, width, height, stride, self.max_edge)?;
        if image.size() == (width, height) {
            write_image(&self.queue, &image.texture, rgba, width, height, stride);
        } else {
            *image = create_image(&self.device, &self.queue, rgba, width, height, stride);
        }
        Ok(())
    }

    /// Creates a render target of `width` x `height` pixels.
    pub fn create_target(&self, width: u32, height: u32) -> Result<Target, RenderError> {
        if width == 0 || height == 0 || width > self.max_edge || height > self.max_edge {
            return Err(RenderError::InvalidSize(width, height));
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wprs-render target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wprs-render readback"),
            size: u64::from(padded_row_bytes(width)) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Ok(Target {
            texture,
            view,
            readback,
            width,
            height,
        })
    }

    /// Renders `scene` into a new `width` x `height` frame.
    pub fn render(&self, scene: Scene<'_>, width: u32, height: u32) -> Result<Frame, RenderError> {
        let target = self.create_target(width, height)?;
        let image = scene
            .image
            .map(|img| self.upload(img, scene.fit, (width, height)));
        self.render_layers(
            &target,
            &[Layer {
                image: image.as_ref(),
                fit: scene.fit,
                background: scene.background,
                opacity: 1.0,
            }],
        )
    }

    /// Draws `layers` into `target` and reads the result back.
    pub fn render_layers(&self, target: &Target, layers: &[Layer<'_>]) -> Result<Frame, RenderError> {
        let (width, height) = target.size();
        let bind_groups: Vec<(wgpu::BindGroup, f32)> = layers
            .iter()
            .filter(|l| l.opacity > 0.0)
            .map(|l| (self.layer_bind_group(l, (width, height)), l.opacity.min(1.0)))
            .collect();

        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let padded = padded_row_bytes(width);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wprs-render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            for (i, (bind_group, opacity)) in bind_groups.iter().enumerate() {
                if i == 0 || *opacity >= 1.0 {
                    pass.set_pipeline(&self.opaque);
                } else {
                    pass.set_pipeline(&self.blended);
                    let c = f64::from(*opacity);
                    pass.set_blend_constant(wgpu::Color {
                        r: c,
                        g: c,
                        b: c,
                        a: c,
                    });
                }
                pass.set_bind_group(0, bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &target.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);

        let slice = target.readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| RenderError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| RenderError::Readback(e.to_string()))?
            .map_err(|e| RenderError::Readback(e.to_string()))?;

        let stride = width * 4;
        let mut data = Vec::with_capacity((stride * height) as usize);
        {
            let mapped = slice
                .get_mapped_range()
                .map_err(|e| RenderError::Readback(e.to_string()))?;
            for row in mapped.chunks_exact(padded as usize) {
                data.extend_from_slice(&row[..stride as usize]);
            }
        }
        target.readback.unmap();
        Ok(Frame {
            width,
            height,
            stride,
            data,
        })
    }

    fn layer_bind_group(&self, layer: &Layer<'_>, target: (u32, u32)) -> wgpu::BindGroup {
        let (uv, has_image) = match layer.image {
            Some(img) => (UvRect::compute(layer.fit, img.size(), target), 1),
            None => (UvRect::compute(Fit::Stretch, (1, 1), (1, 1)), 0),
        };
        let bg = layer.background.map(srgb_to_linear);
        let params = Params {
            offset: uv.offset,
            scale: uv.scale,
            background: [bg[0], bg[1], bg[2], 1.0],
            flags: [uv.repeat as u32, has_image, 0, 0],
        };
        let uniform = wgpu::util_buffer(&self.device, bytemuck::bytes_of(&params));
        let view = &layer.image.unwrap_or(&self.placeholder).view;
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(if uv.repeat {
                        &self.repeat
                    } else {
                        &self.clamp
                    }),
                },
            ],
        })
    }
}

fn check_pixels(rgba: &[u8], width: u32, height: u32, stride: u32, max_edge: u32) -> Result<(), RenderError> {
    let needed = u64::from(stride) * u64::from(height.saturating_sub(1)) + u64::from(width) * 4;
    if width == 0
        || height == 0
        || width > max_edge
        || height > max_edge
        || stride < width * 4
        || (rgba.len() as u64) < needed
    {
        return Err(RenderError::InvalidSize(width, height));
    }
    Ok(())
}

fn create_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: &[u8],
    width: u32,
    height: u32,
    stride: u32,
) -> GpuImage {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wprs-render image"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_image(queue, &texture, rgba, width, height, stride);
    let view = texture.create_view(&Default::default());
    GpuImage {
        texture,
        view,
        width,
        height,
    }
}

fn write_image(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    rgba: &[u8],
    width: u32,
    height: u32,
    stride: u32,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(stride),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

/// Small helper module kept private to the crate, named to read naturally at the call site.
mod wgpu {
    pub use ::wgpu::*;

    /// Creates a uniform buffer initialized with `contents`.
    pub fn util_buffer(device: &Device, contents: &[u8]) -> Buffer {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("wprs-render params"),
            size: contents.len() as u64,
            usage: BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        buffer
            .slice(..)
            .get_mapped_range_mut()
            .expect("buffer was created mapped")
            .copy_from_slice(contents);
        buffer.unmap();
        buffer
    }
}

fn padded_row_bytes(width: u32) -> u32 {
    let unpadded = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    unpadded.div_ceil(align) * align
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Returns the image unchanged when it is small enough, or a downscaled copy.
fn prepare_image<'a>(
    img: &'a RgbaImage,
    fit: Fit,
    target: (u32, u32),
    max_edge: u32,
) -> std::borrow::Cow<'a, RgbaImage> {
    let (iw, ih) = img.dimensions();
    let uv = UvRect::compute(fit, (iw, ih), target);
    // Size the image occupies on the target, in target pixels.
    let shown_w = (target.0 as f32 / uv.scale[0]).ceil().max(1.0);
    let shown_h = (target.1 as f32 / uv.scale[1]).ceil().max(1.0);
    let mut factor = (shown_w / iw as f32).max(shown_h / ih as f32).min(1.0);
    let edge = iw.max(ih) as f32;
    if edge * factor > max_edge as f32 {
        factor = max_edge as f32 / edge;
    }
    // Only resample when the GPU would otherwise minify by more than 2x.
    if factor >= 0.5 && iw <= max_edge && ih <= max_edge {
        return std::borrow::Cow::Borrowed(img);
    }
    let w = ((iw as f32 * factor).round() as u32).max(1);
    let h = ((ih as f32 * factor).round() as u32).max(1);
    std::borrow::Cow::Owned(image::imageops::resize(
        img,
        w,
        h,
        image::imageops::FilterType::Triangle,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_padding() {
        assert_eq!(padded_row_bytes(1), 256);
        assert_eq!(padded_row_bytes(64), 256);
        assert_eq!(padded_row_bytes(65), 512);
    }

    #[test]
    fn small_images_are_not_resampled() {
        let img = RgbaImage::new(100, 100);
        assert!(matches!(
            prepare_image(&img, Fit::Fill, (200, 200), 8192),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn large_images_are_downscaled() {
        let img = RgbaImage::new(4000, 2000);
        let out = prepare_image(&img, Fit::Fit, (400, 400), 8192);
        assert_eq!(out.dimensions(), (400, 200));
    }

    #[test]
    fn images_beyond_device_limits_are_downscaled() {
        let img = RgbaImage::new(300, 100);
        let out = prepare_image(&img, Fit::Center, (1000, 1000), 150);
        assert_eq!(out.dimensions(), (150, 50));
    }
}
