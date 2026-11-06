use crate::{RenderError, Renderer};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub(super) const IN_FLIGHT: u64 = 2;
const NAP: Duration = Duration::from_micros(200);

#[derive(Default)]
pub(super) struct Flight {
    done: Arc<AtomicU64>,
    pub(super) sent: u64,
}

impl Flight {
    pub(super) fn wait(&self, r: &Renderer) -> Result<(), RenderError> {
        while self.sent >= IN_FLIGHT && self.done.load(Ordering::Acquire) <= self.sent - IN_FLIGHT {
            r.polled(wgpu::PollType::Poll)?;
            std::thread::sleep(NAP);
        }
        Ok(())
    }

    pub(super) fn submitted(&mut self, r: &Renderer) {
        self.sent += 1;
        let (flag, id) = (Arc::clone(&self.done), self.sent);
        r.queue.on_submitted_work_done(move || {
            flag.fetch_max(id, Ordering::Release);
        });
    }
}
