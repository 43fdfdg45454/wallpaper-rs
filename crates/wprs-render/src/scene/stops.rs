use bytemuck::{Pod, Zeroable};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

pub(crate) const STOPPED: &str = "the graphics card or the program stopped while it drew this effect";
pub(crate) const UNBUILT: &str =
    "the graphics card or the program stopped twice while it built or drew this effect";
const KEPT: usize = 64;

static STOPS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
static VIEWS: AtomicUsize = AtomicUsize::new(1);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn stop_effects(keys: impl IntoIterator<Item = u64>) {
    let mut stops = lock(&STOPS);
    for key in keys {
        stops.retain(|k| *k != key);
        stops.push(key);
    }
    let old = stops.len().saturating_sub(KEPT);
    stops.drain(..old);
}

pub fn retry_effects() -> usize {
    std::mem::take(&mut *lock(&STOPS)).len() + super::clock::pardon()
}

pub(crate) fn stopped(key: u64) -> bool {
    lock(&STOPS).contains(&key) || wprs_scene::struck(key)
}

pub(crate) fn view() -> usize {
    VIEWS.fetch_add(1, Ordering::Relaxed)
}

pub(crate) const MORE: u64 = std::mem::size_of::<More>() as u64;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct More {
    pub(crate) daytime: f32,
    pub(crate) layer_alpha: f32,
    pub(crate) parallax: [f32; 2],
    pub(crate) layer_color: [f32; 4],
    pub(crate) cursor_last: [f32; 2],
    pub(crate) grid: [f32; 2],
    pub(crate) sizes: [[f32; 4]; 4],
    pub(crate) bands_l: [[f32; 4]; 16],
    pub(crate) bands_r: [[f32; 4]; 16],
}

pub(crate) fn frame(runtime: &wprs_scene::Runtime) -> More {
    let (audio, pointer) = (runtime.audio(), runtime.pointer());
    let bands = |b: &[f32; 64]| std::array::from_fn(|i| std::array::from_fn(|j| b[i * 4 + j]));
    let minute = (runtime.wall().rem_euclid(86_400.0) / 60.0).floor() * 60.0;
    let last = [0, 1].map(|k| pointer.smooth[k] - pointer.velocity[k] * runtime.dt());
    More {
        daytime: (minute / 86_400.0) as f32,
        parallax: runtime.parallax(),
        cursor_last: last,
        bands_l: bands(&audio.left),
        bands_r: bands(&audio.right),
        ..More::zeroed()
    }
}
