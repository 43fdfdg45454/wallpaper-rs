use crate::dmabuf::Image;
use crate::shared::{Slot, lock};
use crate::syncobj::{Fences, sync_meta};
use pipewire::sys::pw_buffer;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_core::Region;

const RETRY_GPU: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Use {
    Out,
    Held,
    Leased,
    Ready,
}

struct Raw(*mut pw_buffer);

unsafe impl Send for Raw {}

struct Buffer {
    image: Image,
    state: Use,
    raw: Raw,
    sent: u64,
    fences: Option<Fences>,
}

#[derive(Default)]
pub struct Pool {
    pub active: bool,
    pub failed: Option<Instant>,
    pub broken: bool,
    buffers: Vec<Option<Buffer>>,
    latest: Option<usize>,
    ready: Option<usize>,
    epoch: u64,
    sent: u64,
    damage: Option<Region>,
}

pub enum Grant {
    Copy,
    Wait,
    Buffer(Lease),
}

pub struct Lease {
    slot: Arc<Slot>,
    index: usize,
    epoch: u64,
    pub(crate) shown: bool,
    pub image: Image,
    pub damage: Option<Region>,
}

impl Lease {
    pub(crate) fn slot(&self) -> Arc<Slot> {
        self.slot.clone()
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        lock(&self.slot.pool).finish(self.index, self.epoch, self.shown, self.damage);
    }
}

impl Pool {
    fn state(&self, index: usize) -> Option<Use> {
        self.buffers.get(index)?.as_ref().map(|b| b.state)
    }

    fn set(&mut self, index: usize, to: Use) {
        if let Some(Some(buffer)) = self.buffers.get_mut(index) {
            buffer.state = to;
        }
    }

    pub fn insert(&mut self, image: Image, buffer: *mut pw_buffer) -> usize {
        self.buffers.push(Some(Buffer { image, state: Use::Out, raw: Raw(buffer), sent: 0, fences: None }));
        self.active = true;
        self.buffers.len() - 1
    }

    pub fn remove(&mut self, index: usize, buffer: *mut pw_buffer) {
        if let Some(entry) =
            self.buffers.get_mut(index).filter(|e| e.as_ref().is_some_and(|b| b.raw.0 == buffer))
        {
            *entry = None;
        }
        if self.buffers.iter().all(Option::is_none) {
            self.clear();
        }
    }

    pub fn gpu(&self) -> bool {
        self.failed.is_none_or(|at| at.elapsed() >= RETRY_GPU)
    }

    pub fn clear(&mut self) {
        *self = Self { failed: self.failed, epoch: self.epoch + 1, ..Self::default() };
    }

    pub fn fence(&mut self, index: usize, fences: Fences) {
        if let Some(Some(buffer)) = self.buffers.get_mut(index) {
            buffer.fences = Some(fences);
        }
    }

    pub fn returned(&mut self, index: usize) {
        if self.state(index) != Some(Use::Out) {
            return;
        }
        self.set(index, Use::Held);
        let Some(Some(buffer)) = self.buffers.get_mut(index) else { return };
        if let Some(fences) = buffer.fences.as_mut() {
            fences.returned(unsafe { sync_meta(buffer.raw.0) }.as_deref());
        }
    }

    pub fn lease(&mut self, slot: &Arc<Slot>) -> Option<Lease> {
        let (index, buffer) = (self.buffers.iter().enumerate())
            .filter_map(|(i, b)| Some((i, b.as_ref()?)))
            .filter(|(i, b)| b.state == Use::Held && Some(*i) != self.latest)
            .filter(|(_, b)| b.fences.as_ref().is_none_or(Fences::free))
            .min_by_key(|(_, b)| b.sent)?;
        let image = buffer.image.clone();
        self.set(index, Use::Leased);
        Some(Lease { slot: slot.clone(), index, epoch: self.epoch, shown: false, image, damage: None })
    }

    fn finish(&mut self, index: usize, epoch: u64, shown: bool, damage: Option<Region>) {
        if epoch != self.epoch || self.state(index) != Some(Use::Leased) {
            return;
        }
        if shown {
            self.damage = wprs_core::damage::join(self.damage, damage);
        }
        if let Some(old) = shown.then(|| self.ready.replace(index)).flatten() {
            self.set(old, Use::Held);
        }
        self.set(index, if shown { Use::Ready } else { Use::Held });
    }

    pub fn next(&mut self, resend: bool) -> Option<(*mut pw_buffer, Option<Region>)> {
        let again = self.latest.filter(|&i| resend && self.state(i) == Some(Use::Held));
        let index = self.ready.take().or(again)?;
        self.sent += 1;
        let (sent, buffer) = (self.sent, self.buffers.get_mut(index)?.as_mut()?);
        (buffer.state, buffer.sent) = (Use::Out, sent);
        if let Some(fences) = buffer.fences.as_mut() {
            fences.sent(unsafe { sync_meta(buffer.raw.0) });
        }
        self.latest = Some(index);
        Some((buffer.raw.0, self.damage.replace(Region::default())))
    }

    pub fn starved(&self) -> bool {
        self.active && self.latest.is_none() && self.ready.is_none()
    }
}

#[cfg(test)]
mod tests;
