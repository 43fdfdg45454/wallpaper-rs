use super::state::volume;
use std::collections::HashMap;
pub(crate) use tex::{Tex, bytes};
use wgpu::{BindGroup, TextureFormat};

mod evict;
mod groups;
mod short;
mod tex;
mod uniforms;

pub(crate) const BUDGET: u64 = 512 << 20;
pub(crate) const HELD: u64 = 2 * BUDGET;

pub(super) const KEEP_FRAMES: u64 = 120;

pub(crate) type Key = [u64; 12];

pub(crate) struct Pool {
    pub(super) free: Vec<(Tex, u64)>,
    pub(crate) busy: Vec<(Tex, bool, u64)>,
    pub(super) groups: HashMap<Key, (BindGroup, u64)>,
    pub(crate) data: Vec<u8>,
    pub(super) buffer: Option<wgpu::Buffer>,
    pub(super) frame_group: Option<(BindGroup, BindGroup)>,
    pub(crate) white: Tex,
    pub(crate) clear: Tex,
    pub(crate) void: Tex,
    pub(crate) frame: u64,
    pub(crate) created: u32,
    pub(super) bytes: u64,
    pub(crate) outside: u64,
    pub(crate) cap: u64,
    pub(crate) lend: u64,
    pub(crate) short: Option<u64>,
    pub(crate) ledger: super::limit::Ledger,
    pub(crate) glow: super::lamps::Glow,
}

impl Pool {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue, _format: TextureFormat) -> Pool {
        let solid = |rgba: [u8; 4]| {
            let tex = Tex::new(device, (1, 1), TextureFormat::Rgba8Unorm);
            let layout =
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) };
            queue.write_texture(tex.texture.as_image_copy(), &rgba, layout, tex.texture.size());
            tex
        };
        let (free, busy, groups, data) = Default::default();
        let (white, clear, void) = (solid([255; 4]), solid([0; 4]), volume(device, queue, 1, &[[0.0; 3]]));
        let (buffer, frame_group, frame, created, bytes) = (None, None, 0, 3, 0);
        let ledger = super::limit::Ledger::default();
        Pool {
            free,
            busy,
            groups,
            data,
            buffer,
            frame_group,
            white,
            clear,
            void,
            frame,
            created,
            bytes,
            outside: 0,
            cap: HELD,
            lend: BUDGET,
            short: None,
            ledger,
            glow: super::lamps::Glow::new(device),
        }
    }

    pub(crate) fn acquire(
        &mut self,
        device: &wgpu::Device,
        size: (u32, u32),
        format: TextureFormat,
    ) -> Option<usize> {
        let size = (size.0.max(1), size.1.max(1));
        let need = bytes(size, format);
        if self.bytes + need > self.lend {
            return self.refuse(self.lend);
        }
        let tomb = self.clear.id;
        let fits = |t: &Tex| t.id != tomb && t.size() == size && t.texture.format() == format;
        let found = self.busy.iter().position(|(t, used, _)| !used && fits(t));
        if let Some(i) = found {
            (self.busy[i].1, self.busy[i].2) = (true, self.frame);
        } else {
            if !self.free.iter().any(|(t, _)| fits(t)) && !self.room(need) {
                return self.refuse(self.cap);
            }
            let tex = self.take(device, size, format);
            self.busy.push((tex, true, self.frame));
        }
        self.bytes += need;
        Some(found.unwrap_or(self.busy.len() - 1))
    }
}
