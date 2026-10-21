use crate::stream::OutputStream;
use crate::{Reply, StreamInfo, VideoFrame};
use pipewire as pw;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SERIAL_TIMEOUT: Duration = Duration::from_secs(2);

struct Pending {
    output: String,
    reply: Reply,
    since: Instant,
    created: bool,
}

#[derive(Default)]
pub struct State {
    streams: HashMap<String, OutputStream>,
    pub serials: HashMap<u32, u64>,
    pending: Vec<Pending>,
}

impl State {
    pub fn ensure(&mut self, core: &pw::core::CoreRc, output: String, width: u32, height: u32, reply: Reply) {
        let reuse = self.streams.get(&output).is_some_and(|s| (s.width, s.height) == (width, height));
        if !reuse {
            self.disconnect(&output);
            let stream = match OutputStream::new(core, &output, width, height) {
                Ok(stream) => stream,
                Err(e) => return reply(Err(format!("cannot create stream: {e}"))),
            };
            self.streams.insert(output.clone(), stream);
        }
        self.pending.push(Pending { output, reply, since: Instant::now(), created: !reuse });
        self.resolve_pending();
    }

    pub fn show(&mut self, output: &str, frame: Arc<VideoFrame>) {
        if let Some(stream) = self.streams.get_mut(output) {
            stream.show(frame);
        }
    }

    pub fn remove(&mut self, output: &str) {
        self.disconnect(output);
        self.resolve_pending();
    }

    pub fn tick(&mut self) {
        self.resolve_pending();
        self.streams.values_mut().for_each(OutputStream::keep_alive);
    }

    pub fn close_streams(&mut self) {
        self.streams.clear();
    }

    pub fn resolve_pending(&mut self) {
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
        let node_id = stream.stream.node_id();
        let serial = self.serials.get(&node_id).copied();
        let waited = pending.since.elapsed();
        if node_id != pw::constants::ID_ANY && (serial.is_some() || waited >= SERIAL_TIMEOUT) {
            let (width, height, created) = (stream.width, stream.height, pending.created);
            return Some(Ok(StreamInfo { node_id, serial: serial.unwrap_or(0), width, height, created }));
        }
        (waited >= SERIAL_TIMEOUT * 3).then(|| Err("PipeWire did not create the stream node".into()))
    }

    fn disconnect(&mut self, output: &str) {
        if let Some(old) = self.streams.remove(output) {
            let _ = old.stream.disconnect();
        }
    }
}
