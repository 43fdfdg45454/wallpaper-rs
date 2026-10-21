//! Exports rendered wallpapers as PipeWire video streams, one per output.
//!
//! The PipeWire main loop runs on a dedicated thread. Other threads talk to it
//! through [`Exporter`], which forwards commands over a PipeWire channel.
//! Each stream is a driver: frames are produced only when a new frame is
//! pushed, when a consumer starts streaming, and on a slow keep-alive tick.

mod params;

use pipewire as pw;
use pw::spa;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Tightly packed BGRA8 frame.
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
}

/// Identifies a stream for consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub node_id: u32,
    /// `object.serial` of the node, or 0 when it could not be determined.
    pub serial: u64,
    pub width: u32,
    pub height: u32,
    /// True when this request created the stream, false when it was reused.
    pub created: bool,
}

/// Callback receiving the result of [`Exporter::ensure_stream`].
pub type Reply = Box<dyn FnOnce(Result<StreamInfo, String>) + Send>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PipeWire error: {0}")]
    PipeWire(String),
    #[error("PipeWire thread exited unexpectedly")]
    ThreadGone,
}

enum Command {
    Ensure {
        output: String,
        width: u32,
        height: u32,
        reply: Reply,
    },
    Frame {
        output: String,
        frame: Arc<VideoFrame>,
    },
    Remove {
        output: String,
    },
    Quit,
}

/// Handle to the PipeWire thread. Dropping it stops the thread.
pub struct Exporter {
    tx: pw::channel::Sender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Exporter {
    /// Connects to the PipeWire daemon on a new thread.
    pub fn start() -> Result<Self, Error> {
        let (tx, rx) = pw::channel::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let thread = std::thread::Builder::new()
            .name("wprs-pipewire".into())
            .spawn(move || {
                if let Err(e) = run(rx, &ready_tx) {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| Error::PipeWire(e.to_string()))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                tx,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(Error::PipeWire(e))
            }
            Err(_) => {
                let _ = thread.join();
                Err(Error::ThreadGone)
            }
        }
    }

    /// Creates the stream for `output`, or returns the existing one when the
    /// size matches. A different size replaces the stream and its serial.
    pub fn ensure_stream(&self, output: &str, width: u32, height: u32, reply: Reply) {
        let cmd = Command::Ensure {
            output: output.to_owned(),
            width,
            height,
            reply,
        };
        if let Err(Command::Ensure { reply, .. }) = self.tx.send(cmd) {
            reply(Err("PipeWire thread is not running".into()));
        }
    }

    /// Replaces the frame shown on `output`.
    pub fn push_frame(&self, output: &str, frame: Arc<VideoFrame>) -> Result<(), Error> {
        self.tx
            .send(Command::Frame {
                output: output.to_owned(),
                frame,
            })
            .map_err(|_| Error::ThreadGone)
    }

    /// Destroys the stream of `output`, if any.
    pub fn remove_stream(&self, output: &str) -> Result<(), Error> {
        self.tx
            .send(Command::Remove {
                output: output.to_owned(),
            })
            .map_err(|_| Error::ThreadGone)
    }
}

impl Drop for Exporter {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Quit);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

const TICK: Duration = Duration::from_millis(500);
const KEEPALIVE: Duration = Duration::from_secs(10);
const SERIAL_TIMEOUT: Duration = Duration::from_secs(2);

struct OutputStream {
    stream: pw::stream::StreamRc,
    _listener: pw::stream::StreamListener<()>,
    width: u32,
    height: u32,
    frame: Rc<RefCell<Option<Arc<VideoFrame>>>>,
    streaming: Rc<RefCell<bool>>,
    last_trigger: Instant,
}

impl OutputStream {
    fn trigger(&mut self) {
        tracing::trace!(streaming = *self.streaming.borrow(), "trigger");
        if *self.streaming.borrow() {
            if let Err(e) = self.stream.trigger_process() {
                tracing::debug!("trigger_process failed: {e}");
            }
            self.last_trigger = Instant::now();
        }
    }
}

struct Pending {
    output: String,
    reply: Reply,
    since: Instant,
    created: bool,
}

#[derive(Default)]
struct State {
    streams: HashMap<String, OutputStream>,
    /// node id -> object.serial, from the registry.
    serials: HashMap<u32, u64>,
    pending: Vec<Pending>,
}

impl State {
    /// Answers every pending request whose node and serial are known, or
    /// whose wait timed out (answered with serial 0).
    fn resolve_pending(&mut self) {
        let mut still = Vec::new();
        for p in self.pending.drain(..) {
            let Some(s) = self.streams.get(&p.output) else {
                (p.reply)(Err(format!("stream for '{}' was removed", p.output)));
                continue;
            };
            let node_id = s.stream.node_id();
            let node_known = node_id != pw::constants::ID_ANY;
            let serial = node_known.then(|| self.serials.get(&node_id).copied()).flatten();
            let timed_out = p.since.elapsed() >= SERIAL_TIMEOUT;
            if node_known && (serial.is_some() || timed_out) {
                (p.reply)(Ok(StreamInfo {
                    node_id,
                    serial: serial.unwrap_or(0),
                    width: s.width,
                    height: s.height,
                    created: p.created,
                }));
            } else if timed_out && p.since.elapsed() >= SERIAL_TIMEOUT * 3 {
                (p.reply)(Err("PipeWire did not create the stream node".into()));
            } else {
                still.push(p);
            }
        }
        self.pending = still;
    }
}

fn run(
    rx: pw::channel::Receiver<Command>,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), pw::Error> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let registry = core.get_registry_rc()?;
    let state = Rc::new(RefCell::new(State::default()));

    let _registry_listener = {
        let state_add = state.clone();
        let state_remove = state.clone();
        registry
            .add_listener_local()
            .global(move |global| {
                if global.type_ != pw::types::ObjectType::Node {
                    return;
                }
                let serial = global
                    .props
                    .and_then(|p| p.get("object.serial"))
                    .and_then(|s| s.parse::<u64>().ok());
                if let Some(serial) = serial
                    && let Ok(mut st) = state_add.try_borrow_mut()
                {
                    st.serials.insert(global.id, serial);
                    st.resolve_pending();
                }
            })
            .global_remove(move |id| {
                if let Ok(mut st) = state_remove.try_borrow_mut() {
                    st.serials.remove(&id);
                }
            })
            .register()
    };

    let _receiver = {
        let state = state.clone();
        let core = core.clone();
        let mainloop_weak = mainloop.downgrade();
        rx.attach(mainloop.loop_(), move |cmd| match cmd {
            Command::Quit => {
                if let Some(ml) = mainloop_weak.upgrade() {
                    ml.quit();
                }
            }
            Command::Ensure {
                output,
                width,
                height,
                reply,
            } => {
                let mut st = state.borrow_mut();
                let reuse = st
                    .streams
                    .get(&output)
                    .is_some_and(|s| s.width == width && s.height == height);
                if !reuse {
                    if let Some(old) = st.streams.remove(&output) {
                        let _ = old.stream.disconnect();
                    }
                    match create_stream(&core, &output, width, height) {
                        Ok(s) => {
                            st.streams.insert(output.clone(), s);
                        }
                        Err(e) => {
                            drop(st);
                            reply(Err(format!("cannot create stream: {e}")));
                            return;
                        }
                    }
                }
                st.pending.push(Pending {
                    output,
                    reply,
                    since: Instant::now(),
                    created: !reuse,
                });
                st.resolve_pending();
            }
            Command::Frame { output, frame } => {
                let mut st = state.borrow_mut();
                if let Some(s) = st.streams.get_mut(&output) {
                    *s.frame.borrow_mut() = Some(frame);
                    s.trigger();
                }
            }
            Command::Remove { output } => {
                let mut st = state.borrow_mut();
                if let Some(old) = st.streams.remove(&output) {
                    let _ = old.stream.disconnect();
                }
                st.resolve_pending();
            }
        })
    };

    let timer = {
        let state = state.clone();
        mainloop.loop_().add_timer(move |_| {
            let mut st = state.borrow_mut();
            st.resolve_pending();
            for s in st.streams.values_mut() {
                if s.last_trigger.elapsed() >= KEEPALIVE {
                    s.trigger();
                }
            }
        })
    };
    timer
        .update_timer(Some(TICK), Some(TICK))
        .into_result()
        .map_err(pw::Error::SpaError)?;

    let _ = ready.send(Ok(()));
    mainloop.run();
    // Disconnect streams before the core goes away.
    state.borrow_mut().streams.clear();
    Ok(())
}

fn create_stream(
    core: &pw::core::CoreRc,
    output: &str,
    width: u32,
    height: u32,
) -> Result<OutputStream, pw::Error> {
    let name = format!("wallpaper-rs.{output}");
    let props = pw::properties::properties! {
        *pw::keys::MEDIA_TYPE => "Video",
        *pw::keys::MEDIA_CATEGORY => "Source",
        *pw::keys::MEDIA_ROLE => "Screen",
        *pw::keys::NODE_NAME => name.as_str(),
        *pw::keys::NODE_DESCRIPTION => format!("wallpaper-rs ({output})"),
    };
    let stream = pw::stream::StreamRc::new(core.clone(), &name, props)?;
    let frame: Rc<RefCell<Option<Arc<VideoFrame>>>> = Rc::new(RefCell::new(None));
    let streaming = Rc::new(RefCell::new(false));

    let listener = {
        let streaming_state = streaming.clone();
        let frame_process = frame.clone();
        let output_name = output.to_owned();
        stream
            .add_local_listener::<()>()
            .state_changed(move |stream, _, old, new| {
                tracing::debug!(output = %output_name, "stream state {old:?} -> {new:?}");
                let is_streaming = matches!(new, pw::stream::StreamState::Streaming);
                *streaming_state.borrow_mut() = is_streaming;
                if is_streaming {
                    let _ = stream.trigger_process();
                }
            })
            .param_changed(move |stream, _, id, param| {
                if id != spa::param::ParamType::Format.as_raw() || param.is_none() {
                    return;
                }
                tracing::trace!("format negotiated, setting buffer parameters");
                let bytes = params::buffers(width, height);
                if let Some(pod) = spa::pod::Pod::from_bytes(&bytes)
                    && let Err(e) = stream.update_params(&mut [pod])
                {
                    tracing::warn!("cannot set buffer parameters: {e}");
                }
            })
            .process(move |stream, _| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    tracing::trace!("process: no buffer available");
                    return;
                };
                let frame = frame_process.borrow();
                let datas = buffer.datas_mut();
                let Some(data) = datas.first_mut() else { return };
                let (data_type, maxsize) = (data.type_(), data.as_raw().maxsize);
                let mapped = data.data().map(|d| d.len());
                tracing::trace!(?data_type, maxsize, ?mapped, "process: buffer");
                let stride = width * 4;
                let written = match (frame.as_ref(), data.data()) {
                    (Some(f), Some(dst)) => copy_frame(f, dst, width, height, stride),
                    (None, Some(dst)) => {
                        // No frame yet: send opaque black.
                        let len = dst.len().min((stride * height) as usize);
                        for px in dst[..len].chunks_exact_mut(4) {
                            px.copy_from_slice(&[0, 0, 0, 255]);
                        }
                        len
                    }
                    _ => 0,
                };
                tracing::trace!(written, "process: frame queued");
                let chunk = data.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.stride_mut() = stride as i32;
                *chunk.size_mut() = written as u32;
            })
            .register()?
    };

    let format = params::enum_format(width, height);
    let mut pods = [spa::pod::Pod::from_bytes(&format).expect("valid format pod")];
    stream.connect(
        spa::utils::Direction::Output,
        None,
        // PipeWire allocates the buffers (memfd) from our Buffers param.
        pw::stream::StreamFlags::DRIVER | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut pods,
    )?;

    Ok(OutputStream {
        stream,
        _listener: listener,
        width,
        height,
        frame,
        streaming,
        last_trigger: Instant::now(),
    })
}

/// Copies `frame` into `dst` row by row, cropping or leaving black where the
/// sizes differ. Returns the number of bytes covered.
fn copy_frame(frame: &VideoFrame, dst: &mut [u8], width: u32, height: u32, stride: u32) -> usize {
    let rows = height.min(frame.height) as usize;
    let row_bytes = (width.min(frame.width) * 4) as usize;
    let total = (stride * height) as usize;
    if dst.len() < total {
        return 0;
    }
    if frame.width != width || frame.height != height {
        dst[..total].fill(0);
    }
    for y in 0..rows {
        let src = &frame.data[y * frame.stride as usize..][..row_bytes];
        dst[y * stride as usize..][..row_bytes].copy_from_slice(src);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32, v: u8) -> VideoFrame {
        VideoFrame {
            width: w,
            height: h,
            stride: w * 4,
            data: vec![v; (w * h * 4) as usize],
        }
    }

    #[test]
    fn copies_matching_frames() {
        let f = frame(4, 3, 7);
        let mut dst = vec![0u8; 4 * 3 * 4];
        assert_eq!(copy_frame(&f, &mut dst, 4, 3, 16), 48);
        assert!(dst.iter().all(|&b| b == 7));
    }

    #[test]
    fn crops_larger_frames_and_pads_smaller_ones() {
        let big = frame(8, 8, 9);
        let mut dst = vec![1u8; 4 * 2 * 4];
        assert_eq!(copy_frame(&big, &mut dst, 4, 2, 16), 32);
        assert!(dst.iter().all(|&b| b == 9));

        let small = frame(2, 1, 5);
        let mut dst = vec![1u8; 4 * 2 * 4];
        copy_frame(&small, &mut dst, 4, 2, 16);
        assert_eq!(&dst[..8], &[5; 8]);
        assert!(dst[8..].iter().all(|&b| b == 0));
    }

    #[test]
    fn refuses_short_destination() {
        let f = frame(4, 4, 1);
        let mut dst = vec![0u8; 10];
        assert_eq!(copy_frame(&f, &mut dst, 4, 4, 16), 0);
    }
}
