use crate::{Error, Frame, Limits, MAX_FAILURES, MAX_OUTPUT, Script, Source};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};
use wprs_scene::ScriptOutputs;

const PATIENCE: Duration = Duration::from_millis(250);
const RETRY: Duration = Duration::from_millis(100);

type Reply = Result<ScriptOutputs, Error>;

unsafe extern "C" {
    fn tzset();
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Request {
    Load {
        sources: Vec<Source>,
        seed: u64,
        #[serde(default)]
        locale: String,
        #[serde(default)]
        labels: serde_json::Value,
    },
    Frame(serde_json::Value),
}

pub fn serve() -> std::io::Result<()> {
    crate::sandbox()?;
    let (mut script, mut out, mut zone) = (None, std::io::stdout().lock(), String::new());
    for line in std::io::stdin().lock().lines() {
        let reply: Reply = match serde_json::from_str(&line?) {
            Ok(Request::Load { sources, seed, locale, labels }) => {
                let labels = if labels.is_object() { labels } else { serde_json::json!({}) };
                Script::load(&sources, seed, (&locale, &labels), Limits::default()).map(|loaded| {
                    script = Some(loaded);
                    ScriptOutputs::default()
                })
            }
            Ok(Request::Frame(frame)) => {
                if let Some(rule) = frame["zone"].as_str().filter(|z| !z.is_empty() && *z != zone) {
                    zone = rule.to_owned();
                    unsafe { std::env::set_var("TZ", &zone) };
                    unsafe { tzset() };
                }
                match &mut script {
                    Some(script) => script.update_json(&frame.to_string()),
                    None => Err(Error::Disabled),
                }
            }
            Err(e) => Err(Error::Host(e.to_string())),
        };
        writeln!(out, "{}", serde_json::to_string(&reply)?)?;
    }
    Ok(())
}

struct Running {
    child: Child,
    input: Option<ChildStdin>,
    handover: Receiver<ChildStdin>,
    replies: Receiver<String>,
    since: Instant,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Host {
    command: (PathBuf, Vec<String>),
    load: String,
    running: Option<Running>,
    failures: u32,
    retry: Instant,
}

impl Host {
    pub fn new(
        program: impl Into<PathBuf>,
        args: &[&str],
        sources: &[Source],
        seed: u64,
        (locale, labels): (&str, &serde_json::Value),
    ) -> Host {
        let load =
            Request::Load { sources: sources.to_vec(), seed, locale: locale.into(), labels: labels.clone() };
        let load = serde_json::to_string(&load).unwrap_or_default();
        let command = (program.into(), args.iter().map(|a| a.to_string()).collect());
        Host { command, load, running: None, failures: 0, retry: Instant::now() }
    }

    pub fn update(&mut self, frame: &Frame) -> Result<Option<ScriptOutputs>, Error> {
        if self.failures >= MAX_FAILURES {
            return Err(Error::Disabled);
        }
        if self.running.is_none() && self.retry <= Instant::now() {
            match self.spawn() {
                Ok(running) => self.running = Some(running),
                Err(e) => return self.fail(&e.to_string()),
            }
        }
        let Some(running) = &mut self.running else { return Ok(None) };
        let outcome = match running.replies.try_recv() {
            Ok(line) => {
                Ok(Some(serde_json::from_str(&line).unwrap_or_else(|e| Err(Error::Host(e.to_string())))))
            }
            Err(TryRecvError::Empty) if running.since.elapsed() > PATIENCE => Err("no answer within 250 ms"),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err("it exited"),
        };
        let outcome = outcome.and_then(|reply: Option<Reply>| {
            if reply.is_some() {
                running.since = Instant::now();
                let input = match &mut running.input {
                    Some(input) => input,
                    empty => empty.insert(running.handover.recv().map_err(|_| "its input closed")?),
                };
                writeln!(input, "{{\"frame\":{}}}", frame.json()).map_err(|_| "its input closed")?;
            }
            Ok(reply)
        });
        match outcome {
            Err(why) => self.fail(why),
            Ok(Some(Err(Error::Disabled))) => {
                self.failures = MAX_FAILURES;
                Err(Error::Disabled)
            }
            Ok(reply) => reply.transpose(),
        }
    }

    fn spawn(&self) -> std::io::Result<Running> {
        let (program, args) = &self.command;
        let mut child =
            Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
        let (Some(mut input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(std::io::Error::other("no pipes to the script host"));
        };
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut output, mut line) = (BufReader::new(output), String::new());
            while (&mut output)
                .take(4 * MAX_OUTPUT as u64)
                .read_line(&mut line)
                .is_ok_and(|_| line.ends_with('\n'))
                && tx.send(std::mem::take(&mut line)).is_ok()
            {}
        });
        let (load, (give, handover)) = (self.load.clone(), mpsc::channel());
        std::thread::spawn(move || writeln!(input, "{load}").map(|()| give.send(input)));
        Ok(Running { child, input: None, handover, replies, since: Instant::now() })
    }

    fn fail<T>(&mut self, why: &str) -> Result<T, Error> {
        self.running = None;
        self.failures += 1;
        self.retry = Instant::now() + RETRY * 2u32.pow(self.failures);
        Err(if self.failures >= MAX_FAILURES { Error::Disabled } else { Error::Host(why.into()) })
    }
}
