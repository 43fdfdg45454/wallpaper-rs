use super::{KEEP_FRAMES, Key, Pool, Tex};
use wgpu::TextureFormat;

impl Pool {
    pub(crate) fn release(&mut self, i: usize) {
        if let Some((tex, used, _)) = self.busy.get_mut(i).filter(|(_, used, _)| *used) {
            *used = false;
            self.bytes -= tex.bytes();
        }
    }

    pub(crate) fn take(&mut self, device: &wgpu::Device, size: (u32, u32), format: TextureFormat) -> Tex {
        let found = self.free.iter().position(|(t, _)| t.size() == size && t.texture.format() == format);
        match found {
            Some(i) => self.free.remove(i).0,
            None => {
                self.created += 1;
                Tex::new(device, size, format)
            }
        }
    }

    pub(crate) fn give(&mut self, tex: Tex) {
        self.free.push((tex, self.frame));
    }

    pub(super) fn room(&mut self, need: u64) -> bool {
        self.free.sort_unstable_by_key(|(t, used)| (std::cmp::Reverse(*used), t.id));
        let mut idle: Vec<usize> = (0..self.busy.len()).filter(|i| self.busy[*i].2 < self.frame).collect();
        idle.sort_unstable_by_key(|i| std::cmp::Reverse(self.busy[*i].2));
        let mut gone = Vec::new();
        let fits = loop {
            if self.held() + self.outside + need <= self.cap {
                break true;
            }
            if let Some((t, _)) = self.free.pop() {
                gone.push(t.id);
                continue;
            }
            let Some(i) = idle.pop() else { break false };
            gone.push(std::mem::replace(&mut self.busy[i], (self.clear.clone(), false, 0)).0.id);
        };
        self.unbind(&gone);
        fits
    }

    pub(crate) fn held(&self) -> u64 {
        let free = self.free.iter().map(|(t, _)| t.bytes());
        free.chain(self.busy.iter().filter(|b| b.0.id != self.clear.id).map(|(t, ..)| t.bytes())).sum()
    }

    pub(crate) fn end_frame(&mut self) {
        let (frame, tomb) = (self.frame, self.clear.id);
        self.busy.iter_mut().for_each(|slot| slot.1 = false);
        let old = |(t, _, used): &mut (Tex, bool, u64)| t.id == tomb || frame - *used >= KEEP_FRAMES;
        if self.busy.iter_mut().any(&old) {
            let idle = self.busy.extract_if(.., old).filter(|(t, ..)| t.id != tomb);
            self.free.extend(idle.map(|(t, _, used)| (t, used)).collect::<Vec<_>>());
        }
        self.free.sort_unstable_by_key(|(t, used)| (std::cmp::Reverse(*used), t.id));
        let busy: u64 = self.busy.iter().map(|(t, ..)| t.bytes()).sum();
        let room = self.cap.saturating_sub(self.outside + busy);
        let (mut kept, mut gone) = (0, Vec::new());
        self.free.retain(|(t, used)| {
            kept += t.bytes();
            let keep = frame - used < KEEP_FRAMES && kept <= room;
            if !keep {
                gone.push(t.id);
            }
            keep
        });
        let stale = |key: &Key| key[1..].iter().any(|id| *id != 0 && gone.contains(id));
        self.groups.retain(|key, (_, used)| frame - *used < KEEP_FRAMES && !stale(key));
        self.data.clear();
        (self.frame, self.bytes) = (frame + 1, 0);
    }

    pub(crate) fn trim(&mut self) {
        self.free.clear();
        self.busy.clear();
        self.groups.clear();
    }

    pub(crate) fn textures(&self) -> usize {
        self.free.len() + self.busy.len()
    }
}

impl super::super::SceneView {
    pub fn texture_bytes(&self) -> u64 {
        self.pool.held() + self.pool.outside
    }
}
