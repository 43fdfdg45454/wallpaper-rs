use pipewire as pw;
use pw::spa;
use spa::param::ParamType;
use spa::param::audio::{AudioFormat, AudioInfoRaw};
use std::cell::{Cell, RefCell};
use std::io::ErrorKind;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::time::{Duration, Instant};

const WAKE: Duration = Duration::from_millis(50);
const AUDIBLE: f32 = 1e-3;
const NAME: &str = "wallpaper-rs audio analysis";
const FATAL: [ErrorKind; 3] = [ErrorKind::BrokenPipe, ErrorKind::ConnectionReset, ErrorKind::NotConnected];

pub(crate) struct Taps {
    producer: rtrb::Producer<f32>,
    rate: Arc<AtomicU32>,
    wake: Box<dyn Fn() + Send>,
    channels: usize,
    woke: Instant,
}

impl Taps {
    pub(crate) fn new(
        producer: rtrb::Producer<f32>,
        rate: Arc<AtomicU32>,
        wake: Box<dyn Fn() + Send>,
    ) -> Taps {
        Taps { producer, rate, wake, channels: 2, woke: Instant::now() }
    }

    fn take(&mut self, bytes: &[u8]) {
        let channels = self.channels;
        let frames = bytes.chunks_exact(4 * channels);
        let slots = self.producer.slots() / 2;
        let Ok(chunk) = self.producer.write_chunk_uninit(frames.len().min(slots) * 2) else { return };
        let sample = |f: &[u8], c: usize| {
            f.get(4 * c..4 * c + 4).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap_or_default()))
        };
        let mut loudest = 0.0f32;
        let samples = frames.flat_map(|f| [sample(f, 0), sample(f, (channels > 1).into())]);
        chunk.fill_from_iter(samples.inspect(|s| loudest = loudest.max(s.abs())));
        if loudest > AUDIBLE && self.woke.elapsed() >= WAKE {
            self.woke = Instant::now();
            (self.wake)();
        }
    }
}

pub(crate) struct Listen {
    stream: pw::stream::StreamRc,
    _listener: pw::stream::StreamListener<()>,
    _core: pw::core::Listener,
    _sinks: (pw::registry::RegistryRc, pw::registry::Listener),
    broken: Rc<Cell<bool>>,
    retarget: Rc<Cell<bool>>,
}

impl Listen {
    pub(crate) fn open(
        context: &pw::context::ContextRc,
        taps: &Rc<RefCell<Taps>>,
    ) -> Result<Listen, pw::Error> {
        let core = context.connect_rc(None)?;
        let broken = Rc::new(Cell::new(false));
        let lost = broken.clone();
        let _core = core
            .add_listener_local()
            .error(move |id, _, res, message| {
                let kind = std::io::Error::from_raw_os_error(-res).kind();
                if id == pw::core::PW_ID_CORE && FATAL.contains(&kind) {
                    tracing::debug!("the audio capture connection failed: {message}");
                    lost.set(true);
                }
            })
            .register();
        let props = pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::MEDIA_NAME => NAME,
            *pw::keys::STREAM_CAPTURE_SINK => "true",
            *pw::keys::STREAM_MONITOR => "true",
            *pw::keys::NODE_PASSIVE => "true",
            *pw::keys::NODE_LATENCY => "1024/48000",
        };
        let (stuck, retarget) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
        let (waiting, found) = (stuck.clone(), retarget.clone());
        let registry = core.get_registry_rc()?;
        let sinks = registry
            .add_listener_local()
            .global(move |global| {
                let sink = global.props.and_then(|p| p.get(*pw::keys::MEDIA_CLASS)) == Some("Audio/Sink");
                found.set(found.get() || (sink && waiting.get()));
            })
            .register();
        let stream = pw::stream::StreamRc::new(core, NAME, props)?;
        let (formats, feed) = (taps.clone(), taps.clone());
        let _listener = stream
            .add_local_listener::<()>()
            .state_changed(move |_, _, _, new| stuck.set(matches!(new, pw::stream::StreamState::Error(_))))
            .param_changed(move |_, _, id, param| {
                let mut info = AudioInfoRaw::new();
                if let Some(param) = param.filter(|_| id == ParamType::Format.as_raw())
                    && info.parse(param).is_ok()
                    && let Ok(mut taps) = formats.try_borrow_mut()
                {
                    taps.channels = info.channels().max(1) as usize;
                    taps.rate.store(info.rate(), Relaxed);
                }
            })
            .process(move |stream, _| {
                let Some(mut buffer) = stream.dequeue_buffer() else { return };
                let Some(data) = buffer.datas_mut().first_mut() else { return };
                let (offset, size) = (data.chunk().offset() as usize, data.chunk().size() as usize);
                let Some(bytes) = data.data().and_then(|d| d.get(offset..offset + size)) else { return };
                if let Ok(mut taps) = feed.try_borrow_mut() {
                    taps.take(bytes);
                }
            })
            .register()?;
        connect(&stream)?;
        Ok(Listen { stream, _listener, _core, _sinks: (registry, sinks), broken, retarget })
    }

    pub(crate) fn broken(&self) -> bool {
        self.broken.get()
    }

    pub(crate) fn retarget(&self) -> bool {
        self.retarget.get()
    }
}

impl Drop for Listen {
    fn drop(&mut self) {
        let _ = self.stream.disconnect();
    }
}

fn connect(stream: &pw::stream::StreamRc) -> Result<(), pw::Error> {
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
    stream.connect(spa::utils::Direction::Input, None, flags, &mut pods)
}
