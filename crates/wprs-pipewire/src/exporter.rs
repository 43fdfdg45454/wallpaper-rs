use crate::shared::lock;
use crate::{Command, Error, Exporter, Reply, StreamInfo, VideoFrame};
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;

impl Exporter {
    pub fn ensure_stream(&self, output: &str, width: u32, height: u32, reply: Reply) {
        let slot = self.shared.insert(output);
        let command = Command::Ensure { slot, output: output.to_owned(), width, height, reply };
        if let Err(Command::Ensure { reply, .. }) = self.tx.send(command) {
            reply(Err("PipeWire thread is not running".into()));
        }
    }

    pub fn remove_stream(&self, output: &str) -> Result<(), Error> {
        self.shared.remove(output);
        self.send(Command::Remove { output: output.to_owned() })
    }

    pub fn push_frame(&self, output: &str, frame: Arc<VideoFrame>) -> Result<(), Error> {
        check(&frame)?;
        self.wake(self.shared.with_slot(output, |slot| slot.replace(frame)))
    }

    pub fn write_frame(&self, output: &str, frame: VideoFrame<&[u8]>) -> Result<(), Error> {
        check(&frame)?;
        self.wake(self.shared.with_slot(output, |slot| slot.write(&frame)))
    }

    pub fn is_streaming(&self, output: &str) -> bool {
        self.shared.with_slot(output, |slot| slot.streaming.load(SeqCst)).unwrap_or(false)
    }

    pub fn stream_info(&self, output: &str) -> Option<StreamInfo> {
        self.shared.with_slot(output, |slot| *lock(&slot.info)).flatten()
    }

    pub fn generation(&self) -> u64 {
        self.shared.generation.load(SeqCst)
    }

    pub fn on_change(&self, notify: impl Fn() + Send + Sync + 'static) -> Result<(), Error> {
        self.shared.notify.set(Box::new(notify)).map_err(|_| Error::ListenerTaken)
    }

    fn wake(&self, needed: Option<bool>) -> Result<(), Error> {
        if needed != Some(true) || self.shared.waking.swap(true, SeqCst) {
            return Ok(());
        }
        self.send(Command::Wake).inspect_err(|_| self.shared.waking.store(false, SeqCst))
    }

    fn send(&self, command: Command) -> Result<(), Error> {
        self.tx.send(command).map_err(|_| Error::ThreadGone)
    }
}

fn check<D: AsRef<[u8]>>(frame: &VideoFrame<D>) -> Result<(), Error> {
    let (width, height, stride, len) = (frame.width, frame.height, frame.stride, frame.data.as_ref().len());
    frame.is_valid().then_some(()).ok_or(Error::InvalidFrame { width, height, stride, len })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Shared;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local!(static ALLOCATIONS: Cell<usize> = const { Cell::new(0) });

    struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            ALLOCATIONS.with(|n| n.set(n.get() + 1));
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static COUNTING: Counting = Counting;

    #[test]
    fn steady_state_frames_do_not_allocate() {
        let (tx, _rx) = pipewire::channel::channel();
        let exporter = Exporter { tx, shared: Arc::new(Shared::default()), thread: None };
        exporter.shared.insert("A").streaming.store(true, SeqCst);
        let pixels = vec![7u8; 256 * 32];
        let padded = || VideoFrame { width: 60, height: 32, stride: 256, data: &pixels[..] };
        let pushed: Vec<_> = (0..3).map(|_| Arc::new(VideoFrame::copy_of(&padded()))).collect();
        exporter.write_frame("A", padded()).unwrap();
        let before = ALLOCATIONS.with(Cell::get);
        for frame in pushed {
            exporter.push_frame("A", frame).unwrap();
            (0..10).for_each(|_| exporter.write_frame("A", padded()).unwrap());
            assert!(exporter.is_streaming("A") && exporter.stream_info("A").is_none());
        }
        assert_eq!(ALLOCATIONS.with(Cell::get), before);
        assert!(exporter.write_frame("A", VideoFrame { stride: 8, ..padded() }).is_err());
    }
}
