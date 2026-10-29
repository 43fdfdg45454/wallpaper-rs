use crate::bus::Pipe;
use crate::mix::{Clip, Mix, RATE, Track};
use crate::{Error, Event, local_uri, lock, make, pipewire};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::{AppSink, AppSrc, AppSrcCallbacks};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak, mpsc};
use std::time::{Duration, Instant};
use wprs_scene::{MAX_SOUND_SECONDS, MAX_SOUND_TOTAL, Voice};

const CHUNK: usize = 1024;
const QUEUED: Duration = Duration::from_millis(100);
const SOURCE: &str = "appsrc://";
const DECODE_WAIT: Duration = Duration::from_secs(30);
const MARGIN: f32 = 0.5;

pub struct Soundtrack {
    pipe: Pipe,
    mix: Arc<Mutex<Mix>>,
}

#[derive(Clone, Default)]
enum Load {
    #[default]
    Idle,
    Queued,
    Ready(Arc<Clip>),
    Failed,
}

#[derive(Default)]
struct Loads {
    clips: Vec<Load>,
    frames: usize,
    waiting: Vec<(usize, Weak<Mutex<Mix>>, usize)>,
}

type Shared = (Vec<PathBuf>, Mutex<Loads>);
type Job = (Weak<Shared>, usize);

#[derive(Clone, Default)]
pub struct Bank(Arc<Shared>);

fn raw(mono: bool) -> gst::Caps {
    let caps = gst::Caps::builder("audio/x-raw").field("format", "S16LE").field("layout", "interleaved");
    let caps = caps.field("rate", RATE as i32);
    if mono { caps.field("channels", gst::IntRange::new(1, 2)) } else { caps.field("channels", 2) }.build()
}

pub fn decode(path: &Path, budget: usize) -> Result<Clip, Error> {
    crate::init()?;
    let uri = local_uri(path)?;
    let header = std::fs::read(path).map_err(|e| Error::Decode(e.to_string()))?;
    let probed = wprs_scene::Clip::probe(&path.to_string_lossy(), &header).map_err(Error::Decode)?;
    let frames = ((probed.seconds.min(MAX_SOUND_SECONDS) + MARGIN) * RATE as f32) as usize;
    let decoder = gst::ElementFactory::make("uridecodebin").property("uri", uri).build();
    let decoder = decoder.map_err(|_| Error::MissingElement("uridecodebin"))?;
    let (convert, resample) = (make("audioconvert")?, make("audioresample")?);
    let sink = AppSink::builder().caps(&raw(true)).sync(false).build();
    let pipeline = gst::Pipeline::new();
    pipeline.add_many([&decoder, &convert, &resample, sink.upcast_ref()]).map_err(Error::state)?;
    gst::Element::link_many([&convert, &resample, sink.upcast_ref()]).map_err(Error::state)?;
    let input = convert.static_pad("sink").expect("audioconvert has a sink pad");
    decoder.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));
        if caps.structure(0).is_some_and(|s| s.name().starts_with("audio/")) && !input.is_linked() {
            let _ = pad.link(&input);
        }
    });
    pipeline.set_state(gst::State::Playing).map_err(Error::state)?;
    let limit = (frames.min(budget), probed.channels as usize);
    let decoded = pull(&sink, &pipeline.bus().expect("a pipeline has a bus"), limit);
    let _ = pipeline.set_state(gst::State::Null);
    decoded.map_err(Error::Decode)
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
    pub fn new(files: Vec<PathBuf>) -> Bank {
        let loads = Loads { clips: vec![Load::Idle; files.len()], ..Loads::default() };
        Bank(Arc::new((files, Mutex::new(loads))))
    }

    fn want(&self, file: usize, mix: &Arc<Mutex<Mix>>, lane: usize) {
        let mut loads = lock(&self.0.1);
        let waiter = (file, Arc::downgrade(mix), lane);
        match loads.clips.get(file).cloned() {
            Some(Load::Ready(clip)) => lock(mix).load(lane, clip),
            Some(Load::Idle) if queue((Arc::downgrade(&self.0), file)) => {
                loads.clips[file] = Load::Queued;
                loads.waiting.push(waiter);
            }
            Some(Load::Queued) => loads.waiting.push(waiter),
            _ => {}
        }
    }

    fn load(&self, file: usize) {
        let (files, loads) = &*self.0;
        let (used, total) = (lock(loads).frames, ((MAX_SOUND_TOTAL + MARGIN) * RATE as f32) as usize);
        let decoded = decode(&files[file], total.saturating_sub(used));
        let mut loads = lock(loads);
        let load = match decoded {
            Ok(clip) => {
                loads.frames += clip.frames();
                Load::Ready(Arc::new(clip))
            }
            Err(e) => {
                tracing::warn!("cannot decode a sound of the scene");
                tracing::debug!(path = %files[file].display(), "sound that cannot be decoded: {e}");
                Load::Failed
            }
        };
        loads.clips[file] = load.clone();
        let (now, later): (Vec<_>, Vec<_>) =
            std::mem::take(&mut loads.waiting).into_iter().partition(|(f, ..)| *f == file);
        loads.waiting = later;
        let Load::Ready(clip) = load else { return };
        for (_, mix, lane) in now {
            mix.upgrade().iter().for_each(|mix| lock(mix).load(lane, clip.clone()));
        }
    }
}

impl Soundtrack {
    pub fn open(
        bank: &Bank,
        lanes: &[(Option<usize>, Track)],
        voices: &[Voice],
        volume: f64,
        sink: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        crate::init()?;
        let mut mix = Mix::new(&lanes.iter().map(|(_, track)| *track).collect::<Vec<_>>());
        mix.set(voices);
        let mix = Arc::new(Mutex::new(mix));
        let playing = |lane: &usize| voices.get(*lane).is_some_and(|v| v.play);
        let (now, later): (Vec<usize>, Vec<usize>) = (0..lanes.len()).partition(playing);
        for lane in now.into_iter().chain(later) {
            lanes[lane].0.iter().for_each(|&file| bank.want(file, &mix, lane));
        }
        let playbin = make("playbin")?;
        playbin.set_property("uri", SOURCE);
        playbin.set_property_from_str("flags", "audio+soft-volume");
        playbin.set_property("volume", volume.clamp(0.0, 1.0));
        playbin.set_property("mute", false);
        let piped = pipewire::sink();
        piped.iter().for_each(|sink| playbin.set_property("audio-sink", sink));
        let feed = mix.clone();
        playbin.connect("source-setup", false, move |args| {
            let source = args[1].get::<gst::Element>().ok()?.downcast::<AppSrc>().ok()?;
            stream(&source, feed.clone());
            None
        });
        if let Some(bin) = playbin.downcast_ref::<gst::Bin>() {
            bin.connect_deep_element_added(|_, _, element| {
                if element.factory().is_some_and(|f| f.name() == "queue") {
                    element.set_property("max-size-time", QUEUED.as_nanos() as u64);
                }
            });
        }
        let pipe =
            Pipe::start(playbin, (SOURCE.into(), Default::default()), Arc::new(sink), piped.is_some())?;
        Ok(Soundtrack { pipe, mix })
    }

    pub fn set(&self, voices: &[Voice]) {
        lock(&self.mix).set(voices);
    }

    pub fn set_playing(&self, playing: bool) {
        self.pipe.set_playing(playing);
    }

    pub fn set_volume(&self, volume: f64) {
        self.pipe.playbin.set_property("volume", volume.clamp(0.0, 1.0));
    }

    pub fn position(&self) -> Option<Duration> {
        self.pipe.playbin.query_position::<gst::ClockTime>().map(Duration::from)
    }

    pub fn playheads(&self) -> (u64, Vec<usize>) {
        let mix = lock(&self.mix);
        (mix.frames, mix.playheads())
    }
}

fn stream(source: &AppSrc, mix: Arc<Mutex<Mix>>) {
    source.set_caps(Some(&raw(false)));
    source.set_format(gst::Format::Time);
    source.set_max_bytes(CHUNK as u64 * 8);
    let mut at = 0u64;
    let callbacks = AppSrcCallbacks::builder().need_data(move |source, _| {
        let Ok(mut buffer) = gst::Buffer::with_size(CHUNK * 4) else { return };
        let data = buffer.get_mut().expect("a new buffer is writable");
        data.set_pts(gst::ClockTime::from_nseconds(at * 1_000_000_000 / u64::from(RATE)));
        data.set_duration(gst::ClockTime::from_nseconds(CHUNK as u64 * 1_000_000_000 / u64::from(RATE)));
        if let Ok(mut map) = data.map_writable() {
            lock(&mix).fill(&mut map);
        }
        at += CHUNK as u64;
        let _ = source.push_buffer(buffer);
    });
    source.set_callbacks(callbacks.build());
}
