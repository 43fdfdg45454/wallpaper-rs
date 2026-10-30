use crate::{Encoder, Error, Frame, Limits, MAX_FAILURES, Script, Setup};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};
use wprs_scene::ScriptOutputs;

const PATIENCE: Duration = Duration::from_millis(250);

type Reply = Result<Option<String>, Error>;

pub struct Local {
    frames: Sender<Vec<u8>>,
    replies: Receiver<Reply>,
    encoder: Encoder,
    waiting: Option<Instant>,
    failures: u32,
    progress: (u64, u64),
}

impl Local {
    pub fn new(setup: &Setup, limits: Limits) -> Local {
        let (frames, inbox) = mpsc::channel::<Vec<u8>>();
        let (outbox, replies) = mpsc::channel();
        let setup = setup.clone();
        let work = move || {
            let mut script = match Script::load(&setup, limits) {
                Ok(script) => script,
                Err(e) => return drop(outbox.send(Err(e))),
            };
            while let Ok(frame) = inbox.recv() {
                if outbox.send(script.run(&frame)).is_err() {
                    break;
                }
            }
            script.destroy();
        };
        let spawned = std::thread::Builder::new().name("wprs-script".into()).spawn(work);
        let failures = if spawned.is_ok() { 0 } else { MAX_FAILURES };
        Local { frames, replies, encoder: Encoder::default(), waiting: None, failures, progress: (0, 0) }
    }

    pub fn progress(&self) -> (u64, u64) {
        self.progress
    }

    pub fn update(&mut self, frame: &Frame) -> Result<Option<ScriptOutputs>, Error> {
        if self.failures >= MAX_FAILURES {
            return Err(Error::Disabled);
        }
        let reply = match (self.waiting, self.replies.try_recv()) {
            (_, Ok(reply)) => Some(reply),
            (Some(at), Err(TryRecvError::Empty)) if at.elapsed() > PATIENCE => {
                self.failures = MAX_FAILURES;
                return Err(Error::Host("no answer within 250 ms".into()));
            }
            (Some(_), Err(TryRecvError::Empty)) => return Ok(None),
            (_, Err(TryRecvError::Disconnected)) | (None, Err(TryRecvError::Empty)) => None,
        };
        self.progress = (self.progress.0 + 1, self.progress.0);
        if self.frames.send(self.encoder.encode(frame).to_vec()).is_err() {
            self.failures = MAX_FAILURES;
            return Err(match reply {
                Some(Err(e)) => e,
                _ => Error::Disabled,
            });
        }
        self.waiting = Some(Instant::now());
        match reply {
            None | Some(Ok(None)) => Ok(None),
            Some(Ok(Some(text))) => {
                serde_json::from_str(&text).map(Some).map_err(|e| Error::Script(e.to_string()))
            }
            Some(Err(Error::Disabled)) => {
                self.failures = MAX_FAILURES;
                Err(Error::Disabled)
            }
            Some(Err(e)) => Err(e),
        }
    }
}
