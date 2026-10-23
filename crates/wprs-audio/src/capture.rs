use crate::{Analyzer, AudioFrame};
use pipewire as pw;
use pw::spa;
use spa::param::ParamType;
use spa::param::audio::{AudioFormat, AudioInfoRaw};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const RING: usize = 16_384;
const WAKE: Duration = Duration::from_millis(50);
const AUDIBLE: f32 = 1e-3;

pub struct Capture {
    quit: pw::channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

pub struct Feed {
    samples: rtrb::Consumer<f32>,
    rate: Arc<AtomicU32>,
}

impl Feed {
    pub fn rate(&self) -> u32 {
        self.rate.load(Relaxed)
    }

    pub fn drain(&mut self, mut sink: impl FnMut(&[f32])) {
        let Ok(chunk) = self.samples.read_chunk(self.samples.slots() & !1) else { return };
        let (a, b) = chunk.as_slices();
        sink(a);
        sink(b);
        chunk.commit_all();
    }
}

impl Capture {
    pub fn start(wake: impl Fn() + Send + 'static) -> Result<(Capture, Feed), String> {
        let (producer, samples) = rtrb::RingBuffer::new(RING);
        let (rate, (quit, rx), (ready_tx, ready)) =
            (Arc::new(AtomicU32::new(48_000)), pw::channel::channel(), mpsc::channel());
        let shared = rate.clone();
        let thread = std::thread::Builder::new()
            .name("wprs-audio".into())
            .spawn(move || {
                if let Err(e) = run(rx, producer, shared, wake, &ready_tx) {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| e.to_string())?;
        let failure = match ready.recv() {
            Ok(Ok(())) => return Ok((Capture { quit, thread: Some(thread) }, Feed { samples, rate })),
            Ok(Err(e)) => e,
            Err(_) => "the audio thread stopped".into(),
        };
        let _ = thread.join();
        Err(failure)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.quit.send(());
        self.thread.take().map(JoinHandle::join);
    }
}

fn run(
    rx: pw::channel::Receiver<()>,
    mut producer: rtrb::Producer<f32>,
    rate: Arc<AtomicU32>,
    wake: impl Fn() + 'static,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), pw::Error> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let props = pw::properties::properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Music",
        *pw::keys::MEDIA_NAME => "wallpaper-rs audio analysis",
        *pw::keys::STREAM_CAPTURE_SINK => "true",
        *pw::keys::STREAM_MONITOR => "true",
        *pw::keys::NODE_PASSIVE => "true",
        *pw::keys::NODE_LATENCY => "1024/48000",
    };
    let stream = pw::stream::StreamRc::new(context.connect_rc(None)?, "wallpaper-rs audio analysis", props)?;
    let channels = Rc::new(Cell::new(2usize));
    let read = channels.clone();
    let mut woke = Instant::now();
    let _listener = stream
        .add_local_listener::<()>()
        .param_changed(move |_, _, id, param| {
            let mut info = AudioInfoRaw::new();
            if let Some(param) = param.filter(|_| id == ParamType::Format.as_raw())
                && info.parse(param).is_ok()
            {
                channels.set(info.channels().max(1) as usize);
                rate.store(info.rate(), Relaxed);
            }
        })
        .process(move |stream, _| {
            let Some(mut buffer) = stream.dequeue_buffer() else { return };
            let Some(data) = buffer.datas_mut().first_mut() else { return };
            let (offset, size, channels) =
                (data.chunk().offset() as usize, data.chunk().size() as usize, read.get());
            let Some(bytes) = data.data().and_then(|d| d.get(offset..offset + size)) else { return };
            let frames = bytes.chunks_exact(4 * channels);
            let Ok(chunk) = producer.write_chunk_uninit(frames.len().min(producer.slots() / 2) * 2) else {
                return;
            };
            let sample = |f: &[u8], c: usize| {
                f.get(4 * c..4 * c + 4).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap_or_default()))
            };
            let mut loudest = 0.0f32;
            let samples = frames.flat_map(|f| [sample(f, 0), sample(f, (channels > 1).into())]);
            chunk.fill_from_iter(samples.inspect(|s| loudest = loudest.max(s.abs())));
            if loudest > AUDIBLE && woke.elapsed() >= WAKE {
                woke = Instant::now();
                wake();
            }
        })
        .register()?;
    let mut info = AudioInfoRaw::new();
    info.set_format(AudioFormat::F32LE);
    let format = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };
    let bytes = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(format),
    )
    .map_err(|_| pw::Error::CreationFailed)?
    .0
    .into_inner();
    let mut pods = [spa::pod::Pod::from_bytes(&bytes).ok_or(pw::Error::CreationFailed)?];
    let flags = pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS;
    stream.connect(spa::utils::Direction::Input, None, flags, &mut pods)?;
    let quit = mainloop.clone();
    let _quit = rx.attach(mainloop.loop_(), move |()| quit.quit());
    let _ = ready.send(Ok(()));
    mainloop.run();
    let _ = stream.disconnect();
    Ok(())
}

pub struct Audio {
    analyzer: Analyzer,
    feed: Option<Feed>,
    last: Option<Instant>,
}

impl Default for Audio {
    fn default() -> Self {
        Audio { analyzer: Analyzer::new(48_000), feed: None, last: None }
    }
}

impl Audio {
    pub fn attach(&mut self, feed: Option<Feed>) {
        *self = Audio { feed, ..Audio::default() };
    }

    pub fn attached(&self) -> bool {
        self.feed.is_some()
    }

    pub fn frame(&mut self) -> AudioFrame {
        let Some(feed) = &mut self.feed else { return AudioFrame::default() };
        self.analyzer.set_rate(feed.rate());
        feed.drain(|samples| self.analyzer.push(samples));
        let now = Instant::now();
        let dt = self.last.replace(now).map_or(0.0, |last| (now - last).as_secs_f32());
        self.analyzer.update(dt).clone()
    }
}
