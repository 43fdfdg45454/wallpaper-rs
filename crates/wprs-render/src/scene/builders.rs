use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(1);

type Builders = Vec<(JoinHandle<()>, Arc<AtomicBool>)>;

static BUILDERS: Mutex<Builders> = Mutex::new(Vec::new());

fn lock() -> MutexGuard<'static, Builders> {
    BUILDERS.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn keep(thread: JoinHandle<()>, stop: Arc<AtomicBool>) {
    let mut builders = lock();
    builders.retain(|(t, _)| !t.is_finished());
    builders.push((thread, stop));
}

pub fn finish_builds(code: i32) {
    let builders = std::mem::take(&mut *lock());
    builders.iter().for_each(|(_, stop)| stop.store(true, Relaxed));
    let until = Instant::now() + BOUND;
    while builders.iter().any(|(t, _)| !t.is_finished()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    if builders.iter().all(|(t, _)| t.is_finished()) {
        return builders.into_iter().for_each(|(t, _)| drop(t.join()));
    }
    tracing::warn!("a package shader is still building; exiting without waiting for the driver");
    drop(std::io::stdout().flush());
    drop(std::io::stderr().flush());
    wprs_scene::end_trials();
    unsafe { libc::_exit(code) }
}
