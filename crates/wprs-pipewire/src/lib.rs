mod exporter;
mod frame;
mod link;
mod listener;
mod main_loop;
mod memfd;
mod params;
mod shared;
mod state;
mod stream;

use pipewire as pw;
use shared::{Shared, Slot};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;

pub use frame::VideoFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub node_id: u32,
    pub serial: u64,
    pub width: u32,
    pub height: u32,
    pub created: bool,
}

pub type Reply = Box<dyn FnOnce(Result<StreamInfo, String>) + Send>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PipeWire error: {0}")]
    PipeWire(String),
    #[error("PipeWire thread exited unexpectedly")]
    ThreadGone,
    #[error("invalid frame {width}x{height} with stride {stride} and {len} bytes")]
    InvalidFrame { width: u32, height: u32, stride: u32, len: usize },
    #[error("a change listener is already registered")]
    ListenerTaken,
}

enum Command {
    Ensure { slot: Arc<Slot>, output: String, width: u32, height: u32, reply: Reply },
    Remove { output: String },
    Wake,
    Quit,
}

pub struct Exporter {
    tx: pw::channel::Sender<Command>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Exporter {
    pub fn start() -> Result<Self, Error> {
        let (tx, rx) = pw::channel::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let shared = Arc::new(Shared::default());
        let for_thread = shared.clone();
        let thread = std::thread::Builder::new()
            .name("wprs-pipewire".into())
            .spawn(move || {
                if let Err(e) = main_loop::run(rx, for_thread, &ready_tx) {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| Error::PipeWire(e.to_string()))?;
        let failure = match ready_rx.recv() {
            Ok(Ok(())) => return Ok(Self { tx, shared, thread: Some(thread) }),
            Ok(Err(e)) => Error::PipeWire(e),
            Err(_) => Error::ThreadGone,
        };
        let _ = thread.join();
        Err(failure)
    }
}

impl Drop for Exporter {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
