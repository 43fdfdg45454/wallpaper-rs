use crate::texture::{create_texture, gpu_image};
use crate::{GpuImage, RenderError, Renderer};
use half::f16;
use std::io::Read;
use std::path::{Path, PathBuf};
use wgpu::{TextureFormat, TextureUsages};
use wprs_ktx2::{MAX_BYTES, Target};

fn packed(path: &Path) -> Option<Vec<u8>> {
    let ktx2 = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ktx2"));
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok().filter(|_| ktx2)?.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

pub fn bright_picture(path: &Path) -> bool {
    let mut head = Vec::new();
    let ktx2 = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ktx2"));
    let opened = std::fs::File::open(path).ok().filter(|_| ktx2);
    opened.and_then(|f| f.take(4096).read_to_end(&mut head).ok()).is_some()
        && wprs_ktx2::is_ktx2(&head)
        && wprs_ktx2::kv::glance(&head).is_some_and(|g| g.hdr)
}

fn kneed([r, g, b]: [f32; 3], top: f32) -> [f32; 3] {
    let k = crate::KNEE;
    let l = r.max(g).max(b);
    let (x, wx) = ((l - k).max(0.0) / (1.0 - k), (top - k) / (1.0 - k));
    let m = if l > k && top > 1.0 { k + (1.0 - k) * x * (1.0 + x / (wx * wx)) / (1.0 + x) } else { l };
    [r, g, b].map(|c| (c * (m / l.max(1e-6))).clamp(0.0, 1.0))
}

pub fn light_texels(half: &mut [u8], headroom: f32) {
    let headroom = if headroom.is_finite() { headroom.clamp(1.0, crate::hdr::MAX_HEADROOM) } else { 1.0 };
    let read = |t: &[u8; 8], i: usize| {
        let v = f16::from_le_bytes([t[2 * i], t[2 * i + 1]]).to_f32();
        if v.is_finite() { v.max(0.0) } else { 0.0 }
    };
    let texels = half.as_chunks_mut::<8>().0;
    let peak = texels.iter().flat_map(|t| [read(t, 0), read(t, 1), read(t, 2)]).fold(0.0, f32::max);
    let top = peak / headroom;
    for t in texels {
        let shown = kneed([read(t, 0), read(t, 1), read(t, 2)].map(|c| c / headroom), top);
        for (i, c) in shown.into_iter().enumerate() {
            t[2 * i..2 * i + 2].copy_from_slice(&f16::from_f32(c * headroom).to_le_bytes());
        }
        let alpha = read(t, 3).min(1.0);
        t[6..8].copy_from_slice(&f16::from_f32(alpha).to_le_bytes());
    }
}

impl Renderer {
    pub fn upload_light(&self, path: &Path, headroom: f32) -> Result<Option<GpuImage>, RenderError> {
        let Some(bytes) = (headroom > 1.0).then_some(path).and_then(packed) else { return Ok(None) };
        let made = wprs_ktx2::levels(&bytes, Target::Half, self.max_edge(), false);
        let Some(mut made) =
            made.ok().filter(|m| m.target == Target::Half && self.limits.fits(m.width, m.height))
        else {
            return Ok(None);
        };
        light_texels(&mut made.data, headroom);
        let (size, usage) = (
            (made.width, made.height),
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
        );
        self.scoped(|| {
            let texture =
                create_texture(&self.device, "wprs-render light", size, TextureFormat::Rgba16Float, usage);
            let layout = wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(made.width * 8),
                rows_per_image: None,
            };
            self.queue.write_texture(texture.as_image_copy(), &made.data, layout, texture.size());
            Some(gpu_image(texture))
        })
    }
}

impl<'a> crate::Layer<'a> {
    pub fn lit(self, image: Option<&'a GpuImage>) -> Self {
        crate::Layer { image: image.or(self.image), ..self }
    }
}

#[derive(Default)]
pub struct Lit {
    seen: Option<PathBuf>,
    bright: bool,
    headroom: f32,
    image: Option<GpuImage>,
}

impl Lit {
    pub fn follow(&mut self, path: Option<&Path>) {
        if path != self.seen.as_deref() {
            let bright = path.is_some_and(bright_picture);
            *self = Lit { seen: path.map(Path::to_owned), bright, ..Lit::default() };
        }
    }

    pub fn bright(&self) -> bool {
        self.bright
    }

    pub fn image(&self) -> Option<&GpuImage> {
        self.image.as_ref()
    }

    pub fn relight(&mut self, r: &Renderer, headroom: f32) -> bool {
        let wanted = if self.bright && headroom > 1.0 { headroom } else { 1.0 };
        if (wanted - self.headroom.max(1.0)).abs() < 1e-3 {
            return false;
        }
        let had = self.image.take().is_some();
        self.headroom = wanted;
        let path = self.seen.as_deref().filter(|_| wanted > 1.0);
        let made = path.map(|path| r.upload_light(path, wanted)).transpose();
        let made = made.inspect_err(|e| tracing::warn!("cannot show the light of the picture: {e}"));
        self.image = made.ok().flatten().flatten();
        had || self.image.is_some()
    }
}
