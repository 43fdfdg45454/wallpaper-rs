use super::decode;
use super::feed::Disk;
use crate::pack::Pack;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use wprs_mix::{Mix, RATE, Source};
use wprs_scene::{MAX_SOUND_TOTAL, Value, Voice};

const MARGIN: f32 = 0.5;

pub(super) static LOADED: AtomicU32 = AtomicU32::new(0);
pub(super) static FAILED: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Default)]
enum Load {
    #[default]
    Idle,
    Queued,
    Ready(Source),
    Failed,
}

type Waiter = (usize, Weak<Mutex<Mix>>, (usize, usize));

#[derive(Default)]
struct Loads {
    clips: Vec<Load>,
    frames: usize,
    waiting: Vec<Waiter>,
}

pub(super) struct Bank {
    files: Arc<Vec<(PathBuf, bool)>>,
    loads: Arc<Mutex<Loads>>,
}

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn fresh(files: usize) -> Loads {
    Loads { clips: vec![Load::Idle; files], ..Loads::default() }
}

impl Bank {
    pub(super) fn new(pack: &Pack) -> Bank {
        let streamed = |id: &str| pack.scene.sounds().any(|s| s.stream && s.clips().any(|c| c == id));
        let files: Vec<_> = pack.sounds.iter().map(|(id, path, _)| (path.clone(), streamed(id))).collect();
        Bank { loads: Arc::new(Mutex::new(fresh(files.len()))), files: Arc::new(files) }
    }

    pub(super) fn clear(&self) {
        *locked(&self.loads) = fresh(self.files.len());
    }

    pub(super) fn fill(&self, pack: &Pack, mix: &Arc<Mutex<Mix>>, voices: &[Voice]) {
        let file = |id: &str| pack.sounds.iter().position(|(sound, ..)| sound == id);
        let lanes: Vec<Vec<Option<usize>>> = pack
            .scene
            .sounds()
            .map(|s| s.clips().map(|id| file(id).filter(|_| s.play != Value::Fixed(false))).collect())
            .collect();
        let playing = |lane: &usize| voices.get(*lane).is_some_and(|v| v.play);
        let (now, later): (Vec<usize>, Vec<usize>) = (0..lanes.len()).partition(playing);
        let mut todo = Vec::new();
        let mut loads = locked(&self.loads);
        for lane in now.into_iter().chain(later) {
            for (clip, file) in lanes[lane].iter().enumerate() {
                let Some(&file) = file.as_ref() else { continue };
                let waiter = (file, Arc::downgrade(mix), (lane, clip));
                match loads.clips[file].clone() {
                    Load::Ready(source) => locked(mix).load(lane, clip, source),
                    Load::Idle => {
                        loads.clips[file] = Load::Queued;
                        todo.push(file);
                        loads.waiting.push(waiter);
                    }
                    Load::Queued => loads.waiting.push(waiter),
                    Load::Failed => {}
                }
            }
        }
        drop(loads);
        if todo.is_empty() {
            return;
        }
        let (files, loads) = (self.files.clone(), self.loads.clone());
        let worker = std::thread::Builder::new().name("wprs-sound-load".into()).spawn(move || {
            todo.into_iter().for_each(|file| load(&files, &loads, file));
        });
        if let Err(e) = worker {
            log::warn!("cannot decode the sounds of the scene: {e}");
        }
    }
}

fn load(files: &[(PathBuf, bool)], loads: &Mutex<Loads>, file: usize) {
    let (path, streamed) = &files[file];
    let total = ((MAX_SOUND_TOTAL + MARGIN) * RATE as f32) as usize;
    let left = total.saturating_sub(locked(loads).frames);
    let loaded = match streamed {
        true => wprs_scene::Clip::file(path).map(|_| Source::Stream(Arc::new(Disk(path.clone())))),
        false => decode::clip(path, left).map(|clip| Source::Memory(Arc::new(clip))),
    };
    let mut loads = locked(loads);
    let load = match loaded {
        Ok(source) => {
            LOADED.fetch_add(1, Relaxed);
            loads.frames += if let Source::Memory(clip) = &source { clip.frames() } else { 0 };
            Load::Ready(source)
        }
        Err(e) => {
            FAILED.fetch_add(1, Relaxed);
            log::warn!("cannot decode a sound of the scene");
            log::debug!("sound that cannot be decoded: {e}");
            Load::Failed
        }
    };
    if let Some(slot) = loads.clips.get_mut(file) {
        *slot = load.clone();
    }
    let (now, later): (Vec<_>, Vec<_>) =
        std::mem::take(&mut loads.waiting).into_iter().partition(|(f, ..)| *f == file);
    loads.waiting = later;
    let Load::Ready(source) = load else { return };
    for (_, mix, (lane, clip)) in now {
        mix.upgrade().iter().for_each(|mix| locked(mix).load(lane, clip, source.clone()));
    }
}
