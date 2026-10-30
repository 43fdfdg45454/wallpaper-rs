use crate::log_allowed;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

pub(crate) type Lost = Arc<(AtomicBool, Mutex<Option<Box<dyn Fn(String) + Send>>>)>;

pub(crate) fn watch(device: &wgpu::Device) -> Lost {
    let lost: Lost = Arc::default();
    let flag = lost.clone();
    device.set_device_lost_callback(move |reason, message| {
        if reason != wgpu::DeviceLostReason::Destroyed {
            lose(&flag, format!("{reason:?}: {message}"));
        }
    });
    static GPU_ERRORS: AtomicU64 = AtomicU64::new(0);
    let flag = lost.clone();
    device.on_uncaptured_error(Arc::new(move |e| {
        if log_allowed(&GPU_ERRORS) {
            tracing::error!("GPU error: {e}");
        }
        if matches!(e, wgpu::Error::OutOfMemory { .. }) || broken(&e) {
            lose(&flag, format!("the device stopped working: {e}"));
        }
    }));
    lost
}

pub(crate) fn broken(e: &wgpu::Error) -> bool {
    let text = e.to_string();
    text.contains("Parent device is lost") || text.contains("label is invalid")
}

pub(crate) fn lose(flag: &Lost, reason: String) {
    if !flag.0.swap(true, Ordering::Relaxed) {
        let call = flag.1.lock().unwrap_or_else(PoisonError::into_inner);
        call.iter().for_each(|call| call(reason.clone()));
    }
}

pub(crate) fn caught<T>(device: &wgpu::Device, work: impl FnOnce() -> T) -> (T, Option<wgpu::Error>) {
    let filters =
        [wgpu::ErrorFilter::OutOfMemory, wgpu::ErrorFilter::Validation, wgpu::ErrorFilter::Internal];
    let scopes = filters.map(|f| device.push_error_scope(f));
    let value = work();
    let popped: Vec<_> = scopes.into_iter().rev().map(|s| s.pop()).collect();
    (value, popped.into_iter().find_map(crate::device::ready))
}

pub(crate) enum Polled {
    Done,
    Timeout,
    Lost(String),
}

impl crate::Renderer {
    #[doc(hidden)]
    pub fn destroy_device(&self) {
        self.device.destroy();
        lose(&self.lost, "the device was destroyed".into());
    }

    pub(crate) fn poll(&self, wait: wgpu::PollType) -> Polled {
        if self.lost() {
            return Polled::Lost("the graphics device was lost".into());
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.device.poll(wait))) {
            Ok(Ok(_)) => Polled::Done,
            Ok(Err(wgpu::PollError::Timeout)) => Polled::Timeout,
            Ok(Err(e)) => {
                lose(&self.lost, e.to_string());
                Polled::Lost(e.to_string())
            }
            Err(_) => {
                let why = "the graphics device was lost while the program waited for it";
                lose(&self.lost, why.into());
                Polled::Lost(why.into())
            }
        }
    }

    pub(crate) fn polled(&self, wait: wgpu::PollType) -> Result<(), crate::RenderError> {
        match self.poll(wait) {
            Polled::Done => Ok(()),
            Polled::Timeout => Err(crate::RenderError::DeviceLost(wgpu::PollError::Timeout.to_string())),
            Polled::Lost(why) => Err(crate::RenderError::DeviceLost(why)),
        }
    }
}
