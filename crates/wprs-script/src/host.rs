use crate::wire::{receive, send};
use crate::{Encoder, Error, Frame, Limits, MAX_FAILURES, Script, Setup, patched};
use std::io::BufReader;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};
use wprs_scene::ScriptOutputs;

const PATIENCE: Duration = Duration::from_millis(250);
const RETRY: Duration = Duration::from_millis(100);
const GRACE: Duration = Duration::from_millis(50);

pub fn serve() -> std::io::Result<()> {
    crate::sandbox()?;
    let (mut input, mut out) = (std::io::stdin().lock(), std::io::stdout().lock());
    let (mut script, mut buf): (Option<Script>, Vec<u8>) = (None, Vec::new());
    while let Some(tag) = receive(&mut input, &mut buf)? {
        let reply = match tag {
            b'L' => serde_json::from_slice::<Setup>(&buf)
                .map_err(|e| Error::Host(e.to_string()))
                .and_then(|setup| Script::load(&setup, Limits::default()))
                .map(|loaded| script = Some(loaded.following_zone()))
                .map(|()| None),
            b'F' => script.as_mut().ok_or(Error::Disabled).and_then(|s| s.run(&buf)),
            _ => Err(Error::Host("an unknown message".into())),
        };
        match reply {
            Ok(None) => send(&mut out, 0, &[])?,
            Ok(Some(text)) => send(&mut out, 1, text.as_bytes())?,
            Err(e) => send(&mut out, 2, serde_json::to_string(&e)?.as_bytes())?,
        }
    }
    script.iter_mut().for_each(Script::destroy);
    Ok(())
}

struct Running {
    child: Option<Child>,
    input: Option<ChildStdin>,
    handover: Receiver<ChildStdin>,
    replies: Receiver<(u8, Vec<u8>)>,
    since: Instant,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.input.take();
        let Some(mut child) = self.child.take() else { return };
        std::thread::spawn(move || {
            let end = Instant::now() + GRACE;
            while child.try_wait().is_ok_and(|done| done.is_none()) && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(5));
            }
            let _ = child.kill();
            let _ = child.wait();
        });
    }
}

pub struct Host {
    command: (PathBuf, Vec<String>),
    load: Vec<u8>,
    running: Option<Running>,
    failures: u32,
    retry: Instant,
    encoder: Encoder,
    progress: (u64, u64),
    kept: ScriptOutputs,
}

impl Host {
    pub fn new(program: impl Into<PathBuf>, args: &[&str], setup: &Setup) -> Host {
        let load = serde_json::to_vec(setup).unwrap_or_default();
        let command = (program.into(), args.iter().map(|a| a.to_string()).collect());
        let (retry, encoder) = (Instant::now(), Encoder::default());
        let kept = ScriptOutputs::default();
        Host { command, load, running: None, failures: 0, retry, encoder, progress: (0, 0), kept }
    }

    pub fn progress(&self) -> (u64, u64) {
        self.progress
    }

    pub fn update(&mut self, frame: &Frame) -> Result<Option<ScriptOutputs>, Error> {
        if self.failures >= MAX_FAILURES {
            return Err(Error::Disabled);
        }
        if self.running.is_none() && self.retry <= Instant::now() {
            match self.spawn() {
                Ok(running) => {
                    self.running = Some(running);
                    self.encoder.reset();
                    self.kept = ScriptOutputs::default();
                    self.progress.1 = self.progress.0;
                }
                Err(e) => return self.fail(&e.to_string()),
            }
        }
        let Some(running) = &mut self.running else { return Ok(None) };
        let outcome = match running.replies.try_recv() {
            Ok(reply) => Ok(Some(reply)),
            Err(TryRecvError::Empty) if running.since.elapsed() > PATIENCE => Err("no answer within 250 ms"),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err("it exited"),
        };
        let outcome = outcome.and_then(|reply| {
            if reply.is_some() {
                self.progress = (self.progress.0 + 1, self.progress.0);
                running.since = Instant::now();
                let input = match &mut running.input {
                    Some(input) => input,
                    empty => empty.insert(running.handover.recv().map_err(|_| "its input closed")?),
                };
                send(input, b'F', self.encoder.encode(frame)).map_err(|_| "its input closed")?;
            }
            Ok(reply)
        });
        match outcome {
            Err(why) => self.fail(why),
            Ok(None) | Ok(Some((0, _))) => Ok(None),
            Ok(Some((1, body))) => {
                let text = String::from_utf8_lossy(&body);
                patched(&mut self.kept, &text, &mut self.encoder).map(|()| Some(self.kept.clone()))
            }
            Ok(Some((_, body))) => match serde_json::from_slice(&body) {
                Ok(Error::Disabled) => {
                    self.failures = MAX_FAILURES;
                    Err(Error::Disabled)
                }
                Ok(e) => Err(e),
                Err(e) => Err(Error::Host(e.to_string())),
            },
        }
    }

    fn spawn(&self) -> std::io::Result<Running> {
        let (program, args) = &self.command;
        let mut command = Command::new(program);
        command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped());
        let orphaned = || match unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } {
            0 => Ok(()),
            _ => Err(std::io::Error::last_os_error()),
        };
        let mut child = unsafe { command.pre_exec(orphaned) }.spawn()?;
        let (Some(mut input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(std::io::Error::other("no pipes to the script host"));
        };
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut output, mut buf) = (BufReader::new(output), Vec::new());
            while let Ok(Some(tag)) = receive(&mut output, &mut buf) {
                if tx.send((tag, std::mem::take(&mut buf))).is_err() {
                    break;
                }
            }
        });
        let (load, (give, handover)) = (self.load.clone(), mpsc::channel());
        std::thread::spawn(move || send(&mut input, b'L', &load).map(|()| give.send(input)));
        Ok(Running { child: Some(child), input: None, handover, replies, since: Instant::now() })
    }

    fn fail<T>(&mut self, why: &str) -> Result<T, Error> {
        self.running = None;
        self.failures += 1;
        self.retry = Instant::now() + RETRY * 2u32.pow(self.failures);
        Err(if self.failures >= MAX_FAILURES { Error::Disabled } else { Error::Host(why.into()) })
    }
}
