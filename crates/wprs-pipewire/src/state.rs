use crate::shared::{Shared, Slot, lock};
use crate::stream::OutputStream;
use crate::{Reply, StreamInfo};
use pipewire as pw;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;
use std::time::{Duration, Instant};

const REPLY_TIMEOUT: Duration = Duration::from_secs(6);

struct Pending {
    output: String,
    reply: Reply,
    since: Instant,
    created: bool,
}

pub struct State {
    shared: Arc<Shared>,
    core: Option<pw::core::CoreRc>,
    streams: HashMap<String, OutputStream>,
    pub serials: HashMap<u32, u64>,
    pending: Vec<Pending>,
}

impl State {
    pub fn new(shared: Arc<Shared>) -> Self {
        Self { shared, core: None, streams: HashMap::new(), serials: HashMap::new(), pending: Vec::new() }
    }

    pub fn ensure(&mut self, slot: Arc<Slot>, output: String, width: u32, height: u32, reply: Reply) {
        if width == 0 || height == 0 {
            return reply(Err(format!("invalid size {width}x{height}")));
        }
        let usable =
            |s: &OutputStream| (s.width, s.height) == (width, height) && (s.is_live() || self.core.is_none());
        let reuse = self.streams.get(&output).is_some_and(usable);
        if !reuse {
            self.drop_stream(&output);
            slot.frame().take_if(|f| (f.width, f.height) != (width, height));
            let mut stream = OutputStream::new(slot, width, height);
            let failed =
                self.core.as_ref().and_then(|core| stream.connect(core, &self.shared, &output).err());
            self.streams.insert(output.clone(), stream);
            if let Some(e) = failed {
                return reply(Err(format!("cannot create stream: {e}")));
            }
        }
        self.pending.push(Pending { output, reply, since: Instant::now(), created: !reuse });
        self.resolve();
    }

    pub fn remove(&mut self, output: &str) {
        self.drop_stream(output);
        self.resolve();
    }

    pub fn wake(&mut self) {
        self.shared.waking.store(false, SeqCst);
        self.streams.values_mut().filter(|s| s.slot.dirty.load(SeqCst)).for_each(OutputStream::trigger);
    }

    pub fn linked(&mut self, node: u32) {
        self.streams.values_mut().for_each(|s| s.linked(node));
    }

    pub fn tick(&mut self) {
        self.resolve();
        for (output, stream) in &mut self.streams {
            stream.keep_alive();
            if let Some(core) = &self.core
                && stream.recover(core, &self.shared, output)
            {
                lock(&self.shared.recreated).push(output.clone());
            }
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        let replies = self.pending.iter().map(|p| p.since + REPLY_TIMEOUT);
        let streams = self.streams.values().flat_map(|s| [s.serial_deadline(), s.keepalive_at()]);
        replies.chain(streams.flatten()).min()
    }

    pub fn connect(&mut self, core: pw::core::CoreRc) {
        self.shared.generation.fetch_add(1, SeqCst);
        for (output, stream) in &mut self.streams {
            if let Err(e) = stream.connect(&core, &self.shared, output) {
                tracing::warn!(output, "cannot recreate the stream: {e}");
            }
        }
        self.core = Some(core);
    }

    pub fn disconnect(&mut self) {
        self.streams.values_mut().for_each(|s| s.disconnect(&self.shared));
        self.serials.clear();
        self.core = None;
    }

    pub fn close(&mut self) {
        self.disconnect();
        self.streams.clear();
    }

    pub fn resolve(&mut self) {
        if self.streams.values_mut().filter_map(|s| s.announce(&self.serials)).count() > 0 {
            self.shared.changed.store(true, SeqCst);
        }
        for pending in std::mem::take(&mut self.pending) {
            match self.answer(&pending) {
                Some(result) => (pending.reply)(result),
                None => self.pending.push(pending),
            }
        }
    }

    fn answer(&self, pending: &Pending) -> Option<Result<StreamInfo, String>> {
        let Some(stream) = self.streams.get(&pending.output) else {
            return Some(Err(format!("stream for '{}' was removed", pending.output)));
        };
        match *lock(&stream.slot.info) {
            Some(info) => Some(Ok(StreamInfo { created: pending.created, ..info })),
            None => (pending.since.elapsed() >= REPLY_TIMEOUT)
                .then(|| Err("PipeWire did not create the stream node".into())),
        }
    }

    fn drop_stream(&mut self, output: &str) {
        if let Some(mut old) = self.streams.remove(output) {
            old.disconnect(&self.shared);
        }
    }
}
