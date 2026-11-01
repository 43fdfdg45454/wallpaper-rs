use crate::scene::stops::{UNBUILT, stopped};
use crate::scene::{builders, clock, flight, packages};
use std::sync::{Arc, atomic::AtomicBool, mpsc};

const ROOM: usize = 64 << 20;

pub(super) fn left(key: u64) -> Option<String> {
    if stopped(key) {
        return Some(UNBUILT.into());
    }
    clock::slow(key)
}

pub(super) fn within<T: Send + 'static>(
    (key, device): (u64, &wgpu::Device),
    work: impl FnOnce(&wgpu::Device) -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    let device = device.clone();
    let job = move || {
        flight::building(key, true);
        let made = wprs_scene::trying(key).and_then(|()| {
            let (made, failure) = crate::lost::caught(&device, || work(&device));
            failure.map_or(Ok(made), |e| Err(e.to_string()))
        });
        flight::building(key, false);
        wprs_scene::tried(key);
        drop(tx.send(made));
    };
    let thread = std::thread::Builder::new().name("wprs-build".into()).stack_size(ROOM).spawn(job);
    let thread = thread.map_err(|e| format!("cannot start building it: {e}"))?;
    builders::keep(thread, Arc::new(AtomicBool::new(false)));
    let limit = packages::build_timeout();
    rx.recv_timeout(limit).unwrap_or_else(|_| {
        if !flight::built(key) && wprs_scene::strike(&[key]).is_err() {
            tracing::warn!("cannot remember that a particle shader took too long to build");
        }
        Err(format!("its shader took more than {} s to build", limit.as_secs_f32()))
    })
}
