use super::Src;
use super::paint::Painter;
use super::pool::Tex;
use crate::texture::next_id;
use wprs_scene::{EffectDraw, ParamKind};

const TICKS: f32 = 4.0;

pub(crate) struct Sim {
    pub(crate) tex: [Tex; 2],
    pub(crate) front: usize,
    pub(crate) used: u64,
    clock: Option<f32>,
    owed: f32,
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
    pub(crate) fn sim(&mut self, key: usize, size: (u32, u32)) -> usize {
        let (device, format, frame) = (&self.r.device, self.state_format, self.pool.frame);
        if self.states.get(&key).is_none_or(|s| s.tex[0].size() != size) {
            let tex = [Tex::new(device, size, format), Tex::new(device, size, format)];
            self.states.insert(key, Sim { tex, front: 0, used: frame, clock: None, owed: 0.0 });
            self.pool.created += 2;
        }
        let sim = self.states.get_mut(&key).expect("inserted above");
        sim.used = frame;
        sim.front
    }

    pub(crate) fn ticks(&mut self, key: usize, time: f32) -> u32 {
        let Some(sim) = self.states.get_mut(&key) else { return 0 };
        let owed = sim.clock.map_or(1.0, |c| sim.owed + (time - c).max(0.0) * 60.0);
        let n = (owed + 1e-3).floor().min(TICKS);
        (sim.clock, sim.owed) = (Some(time), if owed > TICKS { 0.0 } else { (owed - n).max(0.0) });
        n as u32
    }

    pub(crate) fn flip(&mut self, key: usize) {
        if let Some(sim) = self.states.get_mut(&key) {
            sim.front = 1 - sim.front;
        }
    }

    pub(crate) fn asset(&mut self, e: &EffectDraw<'_>, param: &str, kind: ParamKind) -> Option<Src> {
        let def = e.descriptor.params.iter().find(|p| p.id == param && p.kind == kind)?;
        let id = def.named(e.effect)?;
        let (images, luts) = (self.images, self.luts);
        match kind {
            ParamKind::Lut => luts.iter().position(|(name, _)| name == id).map(Src::Lut),
            _ => images.get(id).map(|(image, _)| self.ext(image)),
        }
    }

    pub(crate) fn lut(&mut self, e: &EffectDraw<'_>) -> Src {
        let found = e.descriptor.params.iter().find(|p| p.kind == ParamKind::Lut);
        found.and_then(|p| self.asset(e, &p.id, ParamKind::Lut)).unwrap_or(Src::Void)
    }
}
