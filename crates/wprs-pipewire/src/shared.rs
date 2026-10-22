use crate::{StreamInfo, VideoFrame};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, RwLock};
use std::time::Instant;

type Notify = Box<dyn Fn() + Send + Sync>;

#[derive(Default)]
pub struct Slot {
    frame: Mutex<Option<Arc<VideoFrame>>>,
    pub info: Mutex<Option<StreamInfo>>,
    pub streaming: AtomicBool,
    pub dirty: AtomicBool,
    pub started: Mutex<Option<Instant>>,
}

impl Slot {
    pub fn frame(&self) -> MutexGuard<'_, Option<Arc<VideoFrame>>> {
        lock(&self.frame)
    }

    pub fn replace(&self, frame: Arc<VideoFrame>) -> bool {
        let _old = self.frame().replace(frame);
        self.dirty.store(true, SeqCst);
        self.streaming.load(SeqCst)
    }

    pub fn write(&self, src: &VideoFrame<&[u8]>) -> bool {
        let mut frame = self.frame();
        match frame.as_mut().and_then(Arc::get_mut) {
            Some(retained) => retained.assign(src),
            None => *frame = Some(Arc::new(VideoFrame::copy_of(src))),
        }
        drop(frame);
        self.dirty.store(true, SeqCst);
        self.streaming.load(SeqCst)
    }
}

#[derive(Default)]
pub struct Shared {
    slots: RwLock<HashMap<String, Arc<Slot>>>,
    pub waking: AtomicBool,
    pub changed: AtomicBool,
    pub generation: AtomicU64,
    pub notify: OnceLock<Notify>,
}

impl Shared {
    pub fn insert(&self, output: &str) -> Arc<Slot> {
        let mut slots = self.slots.write().unwrap_or_else(PoisonError::into_inner);
        slots.entry(output.to_owned()).or_default().clone()
    }

    pub fn remove(&self, output: &str) {
        self.slots.write().unwrap_or_else(PoisonError::into_inner).remove(output);
    }

    pub fn with_slot<R>(&self, output: &str, f: impl FnOnce(&Slot) -> R) -> Option<R> {
        self.slots.read().unwrap_or_else(PoisonError::into_inner).get(output).map(|slot| f(slot))
    }

    pub fn flush(&self) {
        if self.changed.swap(false, SeqCst)
            && let Some(notify) = self.notify.get()
        {
            notify();
        }
    }
}

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn borrowed(data: &[u8]) -> VideoFrame<&[u8]> {
        VideoFrame { width: 2, height: 2, stride: 8, data }
    }

    #[test]
    fn writes_reuse_the_retained_buffer_and_wake_only_when_streaming() {
        let (slot, pixels) = (Slot::default(), [3u8; 16]);
        assert!(!slot.write(&borrowed(&pixels)));
        let retained = slot.frame().as_ref().map(|f| f.data.as_ptr());
        slot.streaming.store(true, SeqCst);
        assert!(slot.write(&borrowed(&[4u8; 16])));
        assert_eq!(slot.frame().as_ref().map(|f| f.data.as_ptr()), retained);
        assert!(slot.dirty.load(SeqCst));
    }

    #[test]
    fn pushed_frames_stay_untouched_while_shared() {
        let slot = Slot::default();
        let pushed = Arc::new(VideoFrame { width: 2, height: 2, stride: 8, data: vec![1u8; 16] });
        slot.replace(pushed.clone());
        slot.write(&borrowed(&[2u8; 16]));
        assert!(pushed.data.iter().all(|&b| b == 1));
        assert!(slot.frame().as_ref().is_some_and(|f| f.data == [2u8; 16]));
    }

    #[test]
    fn slots_and_notifications_follow_the_registry() {
        let (shared, calls) = (Shared::default(), Arc::new(AtomicU64::new(0)));
        let counter = calls.clone();
        assert!(shared.notify.set(Box::new(move || _ = counter.fetch_add(1, SeqCst))).is_ok());
        let slot = shared.insert("A");
        assert!(Arc::ptr_eq(&slot, &shared.insert("A")));
        slot.streaming.store(true, SeqCst);
        assert_eq!(shared.with_slot("A", |s| s.streaming.load(SeqCst)), Some(true));
        shared.remove("A");
        assert_eq!(shared.with_slot("A", |_| ()), None);
        shared.flush();
        shared.changed.store(true, SeqCst);
        shared.flush();
        shared.flush();
        assert_eq!(calls.load(SeqCst), 1);
    }
}
