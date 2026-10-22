use crate::listener::listen;
use crate::shared::{Shared, Slot, lock};
use crate::{StreamInfo, params};
use pipewire as pw;
use pw::core::CoreRc;
use pw::spa;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;
use std::time::{Duration, Instant};

const KEEPALIVE: Duration = Duration::from_secs(10);
const SERIAL_TIMEOUT: Duration = Duration::from_secs(2);

struct Live {
    _listener: pw::stream::StreamListener<()>,
    stream: pw::stream::StreamRc,
    since: Instant,
    last_trigger: Instant,
    announced: bool,
}

pub struct OutputStream {
    pub slot: Arc<Slot>,
    pub width: u32,
    pub height: u32,
    live: Option<Live>,
}

impl OutputStream {
    pub fn new(slot: Arc<Slot>, width: u32, height: u32) -> Self {
        Self { slot, width, height, live: None }
    }

    pub fn connect(&mut self, core: &CoreRc, shared: &Arc<Shared>, output: &str) -> Result<(), pw::Error> {
        let name = format!("wallpaper-rs.{output}");
        let props = pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Source",
            *pw::keys::MEDIA_ROLE => "Screen",
            *pw::keys::NODE_NAME => name.as_str(),
            *pw::keys::NODE_DESCRIPTION => format!("wallpaper-rs ({output})"),
        };
        let stream = pw::stream::StreamRc::new(core.clone(), &name, props)?;
        let _listener = listen(&stream, shared.clone(), self.slot.clone(), output, self.width, self.height)?;
        let format = params::enum_format(self.width, self.height);
        let mut pods = [spa::pod::Pod::from_bytes(&format).expect("valid format pod")];
        let flags = pw::stream::StreamFlags::DRIVER | pw::stream::StreamFlags::ALLOC_BUFFERS;
        stream.connect(spa::utils::Direction::Output, None, flags, &mut pods)?;
        let now = Instant::now();
        self.live = Some(Live { _listener, stream, since: now, last_trigger: now, announced: false });
        Ok(())
    }

    pub fn disconnect(&mut self, shared: &Shared) {
        if let Some(live) = self.live.take() {
            let _ = live.stream.disconnect();
        }
        let had_info = lock(&self.slot.info).take().is_some();
        if self.slot.streaming.swap(false, SeqCst) || had_info {
            shared.changed.store(true, SeqCst);
        }
    }

    pub fn is_live(&self) -> bool {
        self.live.is_some()
    }

    pub fn serial_deadline(&self) -> Option<Instant> {
        let live = self.live.as_ref().filter(|live| !live.announced)?;
        Some(live.since + SERIAL_TIMEOUT).filter(|&at| at > Instant::now())
    }

    pub fn announce(&mut self, serials: &HashMap<u32, u64>) -> Option<StreamInfo> {
        let live = self.live.as_mut().filter(|live| !live.announced)?;
        let node_id = live.stream.node_id();
        let serial = serials.get(&node_id).copied();
        let ready = serial.is_some() || live.since.elapsed() >= SERIAL_TIMEOUT;
        (node_id != pw::constants::ID_ANY && ready).then(|| {
            live.announced = true;
            let (width, height, serial) = (self.width, self.height, serial.unwrap_or(0));
            let info = StreamInfo { node_id, serial, width, height, created: true };
            *lock(&self.slot.info) = Some(info);
            info
        })
    }

    pub fn keepalive_at(&self) -> Option<Instant> {
        let streaming = self.slot.streaming.load(SeqCst);
        self.live.as_ref().filter(|_| streaming).map(|live| live.last_trigger + KEEPALIVE)
    }

    pub fn keep_alive(&mut self) {
        if self.keepalive_at().is_some_and(|at| at <= Instant::now()) {
            self.trigger();
        }
    }

    pub fn trigger(&mut self) {
        let streaming = self.slot.streaming.load(SeqCst);
        let Some(live) = self.live.as_mut().filter(|_| streaming) else { return };
        if let Err(e) = live.stream.trigger_process() {
            tracing::debug!("trigger_process failed: {e}");
        }
        live.last_trigger = Instant::now();
    }
}
