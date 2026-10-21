mod device;
mod draw;
mod shader;
mod texture;

pub use device::Renderer;
pub use draw::Target;
pub use texture::GpuImage;
use wprs_core::Fit;

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
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y * self.stride + x * 4) as usize;
        std::array::from_fn(|k| self.data[i + k])
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Scene<'a> {
    pub image: Option<&'a image::RgbaImage>,
    pub fit: Fit,
    pub background: [u8; 3],
}

#[derive(Clone, Copy)]
pub struct Layer<'a> {
    pub image: Option<&'a GpuImage>,
    pub fit: Fit,
    pub background: [u8; 3],
    pub opacity: f32,
}
