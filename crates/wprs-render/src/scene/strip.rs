use super::item::{Item, Rect, ndc};
use super::paint::{Painter, Target};
use super::pipes::Mode;
use super::{Op, Src};
use crate::Renderer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
use std::time::{Duration, Instant};
use wgpu::TextureFormat as Format;
use wprs_scene::{Affine, Kind};

const ROW: u32 = 512;
const EVERY: Duration = Duration::from_millis(33);
const MAPPED: u8 = 1;
const FAILED: u8 = 2;
const WAIT: Duration = Duration::from_millis(100);
pub const MOST: (u32, u32) = (64, 16);

#[derive(Default)]
pub(crate) struct Strip {
    pub(crate) on: bool,
    buffer: Option<wgpu::Buffer>,
    state: Arc<AtomicU8>,
    asked: Option<(u32, u32)>,
    mapping: bool,
    at: Option<Instant>,
}

fn fit(w: f32, h: f32) -> (u32, u32) {
    let scale = (MOST.0 as f32 / w.max(1.0)).min(MOST.1 as f32 / h.max(1.0)).min(1.0);
    (((w * scale).round() as u32).clamp(1, MOST.0), ((h * scale).round() as u32).clamp(1, MOST.1))
}

fn encoded(v: f32, linear: bool) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let v = match linear {
        true if v <= 0.003_130_8 => v * 12.92,
        true => 1.055 * v.powf(1.0 / 2.4) - 0.055,
        false => v,
    };
    (v * 255.0).round() as u8
}

impl Strip {
    fn due(&self) -> bool {
        self.on && self.asked.is_none() && self.at.is_none_or(|t| t.elapsed() >= EVERY)
    }

    pub(crate) fn read(&mut self, r: &Renderer, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture) {
        let Some((w, h)) = self.asked else { return };
        let usage = wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
        let buffer = self.buffer.get_or_insert_with(|| {
            r.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wprs-scene rgb"),
                size: u64::from(ROW * MOST.1),
                usage,
                mapped_at_creation: false,
            })
        });
        let layout =
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ROW), rows_per_image: Some(h) };
        let to = wgpu::TexelCopyBufferInfo { buffer, layout };
        let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        encoder.copy_texture_to_buffer(texture.as_image_copy(), to, size);
    }

    pub(crate) fn take(&mut self, r: &Renderer, format: Format, srgb: bool) -> Option<(u32, u32, Vec<u8>)> {
        let ((w, h), buffer) = (self.asked?, self.buffer.as_ref()?);
        if !self.mapping {
            let flag = self.state.clone();
            flag.store(0, Relaxed);
            buffer.slice(..).map_async(wgpu::MapMode::Read, move |m| {
                flag.store(if m.is_ok() { MAPPED } else { FAILED }, Relaxed)
            });
            self.mapping = true;
        }
        drop(r.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(WAIT) }));
        let state = self.state.load(Relaxed);
        if state == 0 {
            return None;
        }
        (self.asked, self.mapping) = (None, false);
        let wide = format == Format::Rgba16Float;
        let read = (state == MAPPED).then(|| {
            let data = buffer.slice(..).get_mapped_range().ok()?;
            let pixel = |row: &[u8], x: usize| -> [u8; 3] {
                match wide {
                    true => [0, 1, 2].map(|c| {
                        let at = x * 8 + c * 2;
                        encoded(half::f16::from_le_bytes([row[at], row[at + 1]]).to_f32(), !srgb)
                    }),
                    false => [row[x * 4], row[x * 4 + 1], row[x * 4 + 2]],
                }
            };
            let rows = (0..h as usize).map(|y| &data[y * ROW as usize..(y + 1) * ROW as usize]);
            Some(rows.flat_map(|row| (0..w as usize).flat_map(move |x| pixel(row, x))).collect::<Vec<u8>>())
        });
        buffer.unmap();
        read.flatten().map(|rgb| (w, h, rgb))
    }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn strip(&mut self, i: usize, target: Target) {
        let d = &self.frame.draws[i];
        let Kind::Adjustment { size, rgb: true, .. } = d.layer.kind else { return };
        if !self.pool.strip.due() {
            return;
        }
        let grab = match size {
            Some(size) => {
                let at = Affine::translate([d.world.0[4], d.world.0[5]]);
                let shift = Affine::translate([-d.origin[0] * size[0], -d.origin[1] * size[1]]);
                self.screen.then(at).then(shift).then(Affine::scale(size))
            }
            None => self.stage,
        };
        let [gx, gy, gw, gh] = [grab.0[4], grab.0[5], grab.0[0], grab.0[3]];
        let (x0, y0) = (gx.min(gx + gw).floor(), gy.min(gy + gh).floor());
        let (x1, y1) = (gx.max(gx + gw).ceil(), gy.max(gy + gh).ceil());
        let rect = Rect { x: x0, y: y0, w: (x1 - x0).max(1.0), h: (y1 - y0).max(1.0) };
        let Some(src) = self.under(rect, target) else { return };
        let shape = size.unwrap_or(self.frame.canvas);
        let (w, h) = fit(shape[0], shape[1]);
        let Some(small) = self.acquire((w, h)) else { return self.release(src) };
        let uv = [(gx - rect.x) / rect.w, (gy - rect.y) / rect.h, gw / rect.w, gh / rect.h];
        let mut item = Item { quad: [0.0, 0.0, 1.0, 1.0], uv, color: [1.0; 4], ..Item::default() };
        let whole = Rect { x: 0.0, y: 0.0, w: w as f32, h: h as f32 };
        (item.m0, item.m1) = ndc(whole, whole.affine());
        let slot = self.pool.push(&item);
        let tex = [src, Src::White, Src::Clear];
        self.ops.push(Op::Draw { target: small, entry: "fs_texture", mode: Mode::Replace, slot, tex });
        self.ops.push(Op::Strip { from: small });
        (self.pool.strip.asked, self.pool.strip.at) = (Some((w, h)), Some(Instant::now()));
        self.release(src);
        self.release(small);
    }
}

impl super::SceneView {
    pub fn want_rgb(&mut self, on: bool) -> bool {
        let changed = std::mem::replace(&mut self.pool.strip.on, on) != on;
        if changed && on {
            self.stamp = 0;
            let pool = &mut self.pool;
            self.caches.drain().for_each(|(_, c)| pool.give(c.tex));
        }
        changed
    }

    pub fn rgb_pending(&self) -> bool {
        self.pool.strip.asked.is_some()
    }

    pub fn rgb(&mut self, r: &Renderer) -> Option<(u32, u32, Vec<u8>)> {
        self.pool.strip.take(r, self.format, self.srgb)
    }
}

pub fn lit(scene: &wprs_scene::Scene) -> bool {
    scene.layers().any(|l| matches!(l.kind, Kind::Adjustment { rgb: true, .. }))
}
