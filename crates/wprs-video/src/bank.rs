use crate::feed::decoder;
use crate::mix::{Clip, Mix, RATE, Source};
use crate::{Error, lock};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSink;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak, mpsc};
use std::time::{Duration, Instant};
use wprs_scene::{MAX_SOUND_SECONDS, MAX_SOUND_TOTAL};

const EDGE: u64 = 1 << 20;
const DECODE_WAIT: Duration = Duration::from_secs(30);
const MARGIN: f32 = 0.5;

#[derive(Clone, Default)]
enum Load {
    #[default]
    Idle,
    Queued,
    Ready(Source),
    Failed,
}

#[derive(Default)]
struct Loads {
    clips: Vec<Load>,
    frames: usize,
    waiting: Vec<Waiter>,
}

type Shared = (Vec<(PathBuf, bool)>, Mutex<Loads>);
type Waiter = (usize, Weak<Mutex<Mix>>, (usize, usize));
type Job = (Weak<Shared>, usize);

#[derive(Clone, Default)]
pub struct Bank(Arc<Shared>);

pub fn decode(path: &Path, budget: usize) -> Result<Clip, Error> {
    let header = std::fs::read(path).map_err(|e| Error::Decode(e.to_string()))?;
    let probed = wprs_scene::Clip::probe(&path.to_string_lossy(), &header).map_err(Error::Decode)?;
    let estimated = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
    let seconds = if estimated { MAX_SOUND_SECONDS } else { probed.seconds.min(MAX_SOUND_SECONDS) };
    let frames = ((seconds + MARGIN) * RATE as f32) as usize;
    let (pipeline, sink) = decoder(path, None)?;
    pipeline.set_state(gst::State::Playing).map_err(Error::state)?;
    let limit = (frames.min(budget), probed.channels as usize);
    let decoded = pull(&sink, &pipeline.bus().expect("a pipeline has a bus"), limit);
    let _ = pipeline.set_state(gst::State::Null);
    decoded.map_err(Error::Decode)
}

pub fn probe(path: &Path) -> Result<f32, Error> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).map_err(|e| Error::Decode(e.to_string()))?;
    let size = file.metadata().map_err(|e| Error::Decode(e.to_string()))?.len();
    let mut edge = |from: SeekFrom| {
        let mut bytes = Vec::new();
        file.seek(from).and_then(|_| (&mut file).take(EDGE).read_to_end(&mut bytes)).map(|_| bytes)
    };
    let head = edge(SeekFrom::Start(0)).map_err(|e| Error::Decode(e.to_string()))?;
    let tail = edge(SeekFrom::Start(size.saturating_sub(EDGE))).map_err(|e| Error::Decode(e.to_string()))?;
    let clip =
        wprs_scene::Clip::sample(&path.to_string_lossy(), &head, &tail, size).map_err(Error::Decode)?;
    Ok(clip.seconds)
}

fn pull(sink: &AppSink, bus: &gst::Bus, (frames, channels): (usize, usize)) -> Result<Clip, String> {
    let deadline = Instant::now() + DECODE_WAIT;
    let mut clip = Clip { channels, samples: Vec::with_capacity(frames * channels) };
    while !sink.is_eos() {
        if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(100)) {
            let channels = sample.caps().and_then(|c| c.structure(0)?.get::<i32>("channels").ok());
            clip.channels = channels.unwrap_or(1).clamp(1, 2) as usize;
            let map = sample.buffer().and_then(|b| b.map_readable().ok());
            clip.samples
                .extend(map.iter().flat_map(|m| m.as_chunks::<2>().0).map(|b| i16::from_le_bytes(*b)));
        }
        if let Some(gst::MessageView::Error(e)) =
            bus.pop_filtered(&[gst::MessageType::Error]).as_ref().map(|m| m.view())
        {
            return Err(e.error().to_string());
        }
        if clip.frames() > frames {
            return Err(format!("the sound lasts longer than {:.1} s", frames as f32 / RATE as f32));
        }
        if Instant::now() > deadline {
            return Err(format!("the sound was not decoded within {DECODE_WAIT:?}"));
        }
    }
    (clip.frames() > 0).then_some(clip).ok_or_else(|| "the file holds no sound".into())
}

fn queue(job: Job) -> bool {
    static WORKER: OnceLock<Option<mpsc::Sender<Job>>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        let jobs = move || {
            for (bank, file) in rx {
                bank.upgrade().into_iter().for_each(|bank| Bank(bank).load(file));
            }
        };
        std::thread::Builder::new().name("wprs-sound-load".into()).spawn(jobs).ok().map(|_| tx)
    });
    worker.as_ref().is_some_and(|tx| tx.send(job).is_ok())
}

impl Bank {
    pub fn new(files: Vec<(PathBuf, bool)>) -> Bank {
        let loads = Loads { clips: vec![Load::Idle; files.len()], ..Loads::default() };
        Bank(Arc::new((files, Mutex::new(loads))))
    }

    pub(crate) fn want(&self, file: usize, mix: &Arc<Mutex<Mix>>, at: (usize, usize)) {
        let mut loads = lock(&self.0.1);
        match loads.clips.get(file).cloned() {
            Some(Load::Ready(source)) => lock(mix).load(at.0, at.1, source),
            Some(Load::Idle) if queue((Arc::downgrade(&self.0), file)) => {
                loads.clips[file] = Load::Queued;
                loads.waiting.push((file, Arc::downgrade(mix), at));
            }
            Some(Load::Queued) => loads.waiting.push((file, Arc::downgrade(mix), at)),
            _ => {}
        }
    }

    fn load(&self, file: usize) {
        let (files, loads) = &*self.0;
        let (used, total) = (lock(loads).frames, ((MAX_SOUND_TOTAL + MARGIN) * RATE as f32) as usize);
        let (path, streamed) = &files[file];
        let loaded = match streamed {
            true => probe(path).map(|_| Source::Stream(Arc::new(path.clone()))),
            false => decode(path, total.saturating_sub(used)).map(|clip| Source::Memory(Arc::new(clip))),
        };
        let mut loads = lock(loads);
        let load = match loaded {
            Ok(source) => {
                loads.frames += if let Source::Memory(clip) = &source { clip.frames() } else { 0 };
                Load::Ready(source)
            }
            Err(e) => {
                tracing::warn!("cannot decode a sound of the scene");
                tracing::debug!(path = %path.display(), "sound that cannot be decoded: {e}");
                Load::Failed
            }
        };
        loads.clips[file] = load.clone();
        let (now, later): (Vec<_>, Vec<_>) =
            std::mem::take(&mut loads.waiting).into_iter().partition(|(f, ..)| *f == file);
        loads.waiting = later;
        let Load::Ready(source) = load else { return };
        for (_, mix, (lane, clip)) in now {
            mix.upgrade().iter().for_each(|mix| lock(mix).load(lane, clip, source.clone()));
        }
    }
}
