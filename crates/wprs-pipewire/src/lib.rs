mod frame;
mod main_loop;
mod memfd;
mod params;
mod state;
mod stream;

use pipewire as pw;
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
}

enum Command {
    Ensure { output: String, width: u32, height: u32, reply: Reply },
    Frame { output: String, frame: Arc<VideoFrame> },
    Remove { output: String },
    Quit,
}

pub struct Exporter {
    tx: pw::channel::Sender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Exporter {
    pub fn start() -> Result<Self, Error> {
        let (tx, rx) = pw::channel::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let thread = std::thread::Builder::new()
            .name("wprs-pipewire".into())
            .spawn(move || {
                if let Err(e) = main_loop::run(rx, &ready_tx) {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| Error::PipeWire(e.to_string()))?;
        let failure = match ready_rx.recv() {
            Ok(Ok(())) => return Ok(Self { tx, thread: Some(thread) }),
            Ok(Err(e)) => Error::PipeWire(e),
            Err(_) => Error::ThreadGone,
        };
        let _ = thread.join();
        Err(failure)
    }

    pub fn ensure_stream(&self, output: &str, width: u32, height: u32, reply: Reply) {
        let command = Command::Ensure { output: output.to_owned(), width, height, reply };
        if let Err(Command::Ensure { reply, .. }) = self.tx.send(command) {
            reply(Err("PipeWire thread is not running".into()));
        }
    }

    pub fn push_frame(&self, output: &str, frame: Arc<VideoFrame>) -> Result<(), Error> {
        self.send(Command::Frame { output: output.to_owned(), frame })
    }

    pub fn remove_stream(&self, output: &str) -> Result<(), Error> {
        self.send(Command::Remove { output: output.to_owned() })
    }

    fn send(&self, command: Command) -> Result<(), Error> {
        self.tx.send(command).map_err(|_| Error::ThreadGone)
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
