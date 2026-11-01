use super::{B, Swarms, buffer};
use crate::Renderer;
use crate::readback::{FAILED, MAPPED};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
use wgpu::TextureFormat as Format;
use wprs_scene::{Affine, EmitShape, MAX_POINTS, Refresh, Scene};

pub(crate) const SIDE: u32 = 512;
const ROW: u32 = SIDE * 8;

pub(crate) struct Map {
    pub(super) herd: usize,
    pub(super) part: usize,
    pub(super) emitter: usize,
    pub(super) source: String,
    pub(super) refresh: Refresh,
    pub(super) key: Option<u64>,
    pub(super) at: f32,
    pub(super) buffer: wgpu::Buffer,
    pub(super) state: Arc<AtomicU8>,
    pub(super) asked: Option<Asked>,
    pub(super) sent: bool,
    pub(super) texels: Vec<u8>,
    pub(super) shot: Option<Asked>,
    pub(super) threshold: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct Asked {
    pub(crate) size: (u32, u32),
    pub(crate) place: Affine,
    pub(crate) format: Format,
}

impl Swarms {
    pub(super) fn charts(r: &Renderer, scene: &Scene, herds: &[super::Herd]) -> Vec<Map> {
        let systems: Vec<_> = scene.particles.values().collect();
        let mut out = Vec::new();
        for (herd, h) in herds.iter().enumerate() {
            for (part, s) in h.bake.systems.iter().enumerate() {
                for (emitter, e) in systems[*s].emitters.iter().enumerate().take(8) {
                    let EmitShape::Layer(l) = &e.shape else { continue };
                    out.push(Map {
                        herd,
                        part,
                        emitter,
                        source: l.id.clone(),
                        refresh: l.refresh,
                        key: None,
                        at: f32::NEG_INFINITY,
                        buffer: buffer(r, u64::from(ROW * SIDE), B::MAP_READ | B::COPY_DST),
                        state: Arc::new(AtomicU8::new(0)),
                        asked: None,
                        sent: false,
                        texels: Vec::new(),
                        shot: None,
                        threshold: l.threshold.0,
                    });
                }
            }
        }
        out
    }

    pub(crate) fn due(&self, i: usize, key: u64, time: f32) -> bool {
        let Some(m) = self.maps.get(i).filter(|m| m.asked.is_none()) else { return false };
        match (m.key, m.refresh) {
            (None, _) => true,
            (Some(k), Refresh::OnChange) => k != key && time - m.at >= 0.25,
            _ => false,
        }
    }

    pub(crate) fn read(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        i: usize,
        (w, h): (u32, u32),
    ) {
        let Some(m) = self.maps.get(i) else { return };
        let layout =
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ROW), rows_per_image: Some(h) };
        let to = wgpu::TexelCopyBufferInfo { buffer: &m.buffer, layout };
        let size = wgpu::Extent3d { width: w.min(SIDE), height: h.min(SIDE), depth_or_array_layers: 1 };
        encoder.copy_texture_to_buffer(texture.as_image_copy(), to, size);
    }

    pub(crate) fn sent(&mut self) {
        for m in self.maps.iter_mut() {
            if !std::mem::take(&mut m.sent) {
                continue;
            }
            let flag = m.state.clone();
            m.state.store(0, Relaxed);
            m.buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                flag.store(if r.is_ok() { MAPPED } else { FAILED }, Relaxed)
            });
        }
    }

    pub(super) fn collect(&mut self, r: &Renderer, thresholds: impl Fn(usize, usize, usize) -> Option<f32>) {
        if self.maps.iter().any(|m| m.asked.is_some()) {
            drop(r.poll(wgpu::PollType::Poll));
        }
        for i in 0..self.maps.len() {
            let m = &mut self.maps[i];
            let fresh = m.asked.is_some() && m.state.load(Relaxed) != 0;
            if fresh {
                let asked = m.asked.take();
                if m.state.load(Relaxed) == MAPPED {
                    let (w, h) = asked.map_or((0, 0), |a| a.size);
                    let bytes = if asked.is_some_and(|a| a.format == Format::Rgba16Float) { 8 } else { 4 };
                    if let Ok(data) = m.buffer.slice(..).get_mapped_range() {
                        m.texels.clear();
                        let row = (w * bytes) as usize;
                        (0..h as usize)
                            .for_each(|y| m.texels.extend_from_slice(&data[y * ROW as usize..][..row]));
                        m.shot = asked;
                    }
                }
                m.buffer.unmap();
            }
            let wanted = thresholds(m.herd, m.part, m.emitter).unwrap_or(m.threshold);
            if fresh || (wanted - m.threshold).abs() > 0.5 / 255.0 {
                m.threshold = wanted;
                self.chart(r, i);
            }
        }
    }

    fn chart(&mut self, r: &Renderer, i: usize) {
        let m = &self.maps[i];
        let Some(shot) = m.shot else { return };
        let bake = &mut self.herds[m.herd].bake;
        let start = bake.layers[m.part][m.emitter][0] as usize;
        let count = super::texels::compact(
            &m.texels,
            shot,
            m.threshold,
            &mut bake.points[start..start + 2 * MAX_POINTS],
        );
        bake.layers[m.part][m.emitter][1] = count;
        if let Some(Some(f)) = self.flights.get(m.herd) {
            let at = (start * 8) as u64;
            r.queue.write_buffer(
                &f.points,
                at,
                bytemuck::cast_slice(&bake.points[start..start + 2 * count as usize]),
            );
        }
    }
}
