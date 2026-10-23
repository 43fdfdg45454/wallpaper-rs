use crate::dmabuf::Image;
use crate::shared::{Slot, lock};
use pipewire::sys::pw_buffer;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
}

impl Lease {
    pub(crate) fn slot(&self) -> Arc<Slot> {
        self.slot.clone()
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        lock(&self.slot.pool).finish(self.index, self.epoch, self.shown);
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
        self.buffers.push(Some(Buffer { image, state: Use::Out, raw: Raw(buffer), sent: 0 }));
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

    pub fn returned(&mut self, index: usize) {
        if self.state(index) == Some(Use::Out) {
            self.set(index, Use::Held);
        }
    }

    pub fn lease(&mut self, slot: &Arc<Slot>) -> Option<Lease> {
        let (index, buffer) = (self.buffers.iter().enumerate())
            .filter_map(|(i, b)| Some((i, b.as_ref()?)))
            .filter(|(i, b)| b.state == Use::Held && Some(*i) != self.latest)
            .min_by_key(|(_, b)| b.sent)?;
        let image = buffer.image.clone();
        self.set(index, Use::Leased);
        Some(Lease { slot: slot.clone(), index, epoch: self.epoch, shown: false, image })
    }

    fn finish(&mut self, index: usize, epoch: u64, shown: bool) {
        if epoch != self.epoch || self.state(index) != Some(Use::Leased) {
            return;
        }
        if let Some(old) = shown.then(|| self.ready.replace(index)).flatten() {
            self.set(old, Use::Held);
        }
        self.set(index, if shown { Use::Ready } else { Use::Held });
    }

    pub fn next(&mut self, resend: bool) -> Option<*mut pw_buffer> {
        let again = self.latest.filter(|&i| resend && self.state(i) == Some(Use::Held));
        let index = self.ready.take().or(again)?;
        self.sent += 1;
        let (sent, buffer) = (self.sent, self.buffers.get_mut(index)?.as_mut()?);
        (buffer.state, buffer.sent) = (Use::Out, sent);
        self.latest = Some(index);
        Some(buffer.raw.0)
    }

    pub fn starved(&self) -> bool {
        self.active && self.latest.is_none() && self.ready.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(count: usize) -> (Arc<Slot>, Vec<pw_buffer>) {
        let slot = Arc::new(Slot::default());
        let mut raw: Vec<pw_buffer> = (0..count).map(|_| unsafe { std::mem::zeroed() }).collect();
        let mut pool = lock(&slot.pool);
        for buffer in &mut raw {
            let index = pool.insert(Arc::new(()), buffer);
            pool.returned(index);
        }
        drop(pool);
        (slot, raw)
    }

    fn lease(slot: &Arc<Slot>) -> Option<Lease> {
        lock(&slot.pool).lease(slot)
    }

    fn present(mut lease: Lease) {
        lease.shown = true;
    }

    #[test]
    fn rendering_never_touches_the_shown_buffer_and_the_newest_frame_wins() {
        let (slot, raw) = pool(3);
        assert!(lock(&slot.pool).starved());
        present(lease(&slot).expect("a free buffer"));
        assert!(!lock(&slot.pool).starved());
        assert_eq!(lock(&slot.pool).next(false), Some(raw.as_ptr().cast_mut()));
        lock(&slot.pool).returned(0);
        let [a, b] = [lease(&slot), lease(&slot)].map(|l| l.expect("two free buffers"));
        assert_eq!((a.index, b.index), (1, 2));
        assert!(lease(&slot).is_none());
        drop(a);
        present(b);
        let again = lease(&slot).expect("the cancelled buffer");
        assert_eq!(again.index, 1);
        present(again);
        let pool = lock(&slot.pool);
        assert_eq!((pool.ready, pool.state(2)), (Some(1), Some(Use::Held)));
        drop(pool);
        assert_eq!(lock(&slot.pool).next(true), Some(raw[1..].as_ptr().cast_mut()));
        assert_eq!(lock(&slot.pool).next(false), None);
    }

    #[test]
    fn keep_alives_resend_the_newest_frame_and_old_leases_expire() {
        let (slot, raw) = pool(2);
        present(lease(&slot).expect("a free buffer"));
        let newest = lock(&slot.pool).next(false);
        assert_eq!(lock(&slot.pool).next(true), None);
        lock(&slot.pool).returned(0);
        assert_eq!(lock(&slot.pool).next(true), newest);
        let stale = lease(&slot).expect("the other buffer");
        (0..2).for_each(|i| lock(&slot.pool).remove(i, raw[i..].as_ptr().cast_mut()));
        drop(stale);
        let pool = lock(&slot.pool);
        assert!(pool.buffers.is_empty() && !pool.active && pool.epoch == 1);
    }

    #[test]
    fn the_buffer_given_back_longest_ago_is_drawn_into_first() {
        let (slot, _raw) = pool(3);
        let frame = || {
            let lease = lease(&slot).expect("a free buffer");
            let index = lease.index;
            present(lease);
            lock(&slot.pool).next(false);
            (0..3).for_each(|i| lock(&slot.pool).returned(i));
            index
        };
        assert_eq!([(); 6].map(|()| frame()), [0, 1, 2, 0, 1, 2]);
    }
}
