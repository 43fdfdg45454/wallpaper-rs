use crate::changes::Signal;
use crate::daemon::Daemon;
use crate::store::private_dir;
use crate::thumbs::{Req, Task, Thumbs};
use crate::worker::{Request, Response};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Weak};
use std::time::Duration;

const IDLE: Duration = Duration::from_secs(30);
const PER_FILE: Duration = Duration::from_secs(15);
const MOST: Duration = Duration::from_secs(120);

fn patience(size: u64) -> Duration {
    (PER_FILE + Duration::from_secs(size >> 20)).min(MOST)
}

struct Worker {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Worker {
    fn spawn() -> std::io::Result<Worker> {
        let mut child = Command::new(std::env::current_exe()?)
            .arg("--worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let (stdin, stdout) = (child.stdin.take(), child.stdout.take());
        let (Some(stdin), Some(stdout)) = (stdin, stdout) else {
            return Err(std::io::Error::other("no pipes"));
        };
        let (tx, lines) = std::sync::mpsc::channel();
        std::thread::Builder::new().name("wprs-worker-out".into()).spawn(move || {
            BufReader::new(stdout).lines().map_while(Result::ok).try_for_each(|line| tx.send(line))
        })?;
        Ok(Worker { child, stdin, lines })
    }

    fn ask(&mut self, request: &Request, patience: Duration) -> Result<Response, String> {
        let line = serde_json::to_string(request).map_err(|e| e.to_string())?;
        writeln!(self.stdin, "{line}").map_err(|_| "the decoder stopped".to_owned())?;
        match self.lines.recv_timeout(patience) {
            Ok(line) => serde_json::from_str(&line).map_err(|_| "the decoder sent an invalid reply".into()),
            Err(RecvTimeoutError::Timeout) => {
                Err(format!("decoding took longer than {} s", patience.as_secs()))
            }
            Err(RecvTimeoutError::Disconnected) => Err("the decoder stopped on this file".into()),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn start(daemon: &Arc<Daemon>) {
    let slots = (std::thread::available_parallelism().map_or(1, |n| n.get()) / 2).clamp(1, 4);
    for _ in 0..slots {
        let (thumbs, daemon) = (daemon.thumbs.clone(), Arc::downgrade(daemon));
        let spawned =
            std::thread::Builder::new().name("wprs-thumbs".into()).spawn(move || slot(&thumbs, &daemon));
        spawned.map(drop).unwrap_or_else(|e| tracing::warn!("cannot start a thumbnail thread: {e}"));
    }
}

fn slot(thumbs: &Thumbs, daemon: &Weak<Daemon>) {
    let mut worker: Option<Worker> = None;
    loop {
        let Some(task) = thumbs.next(worker.as_ref().map(|_| IDLE)) else {
            worker = None;
            continue;
        };
        let mut sizes: Vec<u32> = task.reqs.iter().map(Req::size).collect();
        sizes.sort_unstable();
        sizes.dedup();
        let analyze = task.reqs.iter().any(|req| matches!(req, Req::Analyze(_)));
        let done = !analyze && sizes.iter().all(|size| thumbs.cached(&task.item, *size).is_some());
        let paths = sizes.iter().map(|size| (*size, thumbs.path(&task.item, *size))).collect();
        let request = Request { path: task.item.path.clone(), kind: task.item.kind, thumbs: paths };
        let response = match (done, task.item.path.is_file()) {
            (true, _) => Response { written: sizes, ..Response::default() },
            (false, false) => {
                Response { missing: true, failure: Some("the file is missing".into()), ..Response::default() }
            }
            (false, true) => run(&mut worker, &request, patience(task.item.size)),
        };
        match daemon.upgrade() {
            Some(daemon) => daemon.thumbnailed(task, response),
            None => return,
        }
    }
}

fn run(worker: &mut Option<Worker>, request: &Request, patience: Duration) -> Response {
    for (_, path) in &request.thumbs {
        path.parent().map(private_dir).transpose().ok();
    }
    if worker.is_none() {
        match Worker::spawn() {
            Ok(spawned) => *worker = Some(spawned),
            Err(e) => {
                let failure = Some(format!("cannot start the decoder: {e}"));
                return Response { failure, retry: true, ..Response::default() };
            }
        }
    }
    match worker.as_mut().map(|w| w.ask(request, patience)) {
        Some(Ok(response)) => response,
        failed => {
            *worker = None;
            let failure = failed.and_then(Result::err);
            Response { failure, retry: true, ..Response::default() }
        }
    }
}

impl Daemon {
    pub fn thumbnailed(&self, task: Task, mut response: Response) {
        let Task { item, reqs } = task;
        let now = || self.store.read(|lib| lib.item(&item.uid)).ok();
        let library = !item.uid.starts_with(crate::previews::PREVIEW);
        if library && self.thumbs.settle(&item, now().as_ref(), &response) {
            response =
                Response { failure: Some("the item was removed or changed".into()), ..Response::default() };
        }
        let meta = &response.meta;
        let fresh = item.meta.width.is_none() && item.meta.duration_ms.is_none() && item.meta.error.is_none();
        let known = meta.width.is_some() || meta.duration_ms.is_some() || meta.error.is_some();
        if fresh
            && known
            && !response.missing
            && self.store.write(|lib| lib.set_meta(&item.uid, meta)).is_ok()
        {
            self.changes.changed("library", vec![item.uid.clone()]);
        }
        let paths = response.written.iter().map(|size| (*size, self.thumbs.path(&item, *size)));
        let bytes = paths.clone().filter_map(|(_, p)| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
        self.thumbs.added(bytes, self.state().config.library.thumb_cache_mb);
        let reason = response.failure.clone().or_else(|| meta.error.clone());
        let reason = reason.unwrap_or_else(|| "no thumbnail could be made".into());
        let analyzed = known && !response.missing && meta.error.is_none();
        for req in reqs {
            let path = paths.clone().find(|(size, _)| *size == req.size()).map(|(_, p)| p);
            match req {
                Req::Wait(_, tx) => drop(tx.send(path.ok_or_else(|| reason.clone()))),
                Req::Signal(_, size) => {
                    let path = path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    self.changes.send(Signal::Thumbnail(item.uid.clone(), size, path))
                }
                Req::Analyze(tx) => drop(tx.send(analyzed)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decoder_that_stops_leaves_the_file_to_try_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.png");
        std::fs::write(&path, b"png").unwrap();
        let request = Request { path, kind: wprs_library::Kind::Image, thumbs: Vec::new() };
        let mut worker = None;
        let response = run(&mut worker, &request, Duration::from_secs(5));
        assert!(response.retry && response.meta.error.is_none() && response.failure.is_some());
        assert!(worker.is_none());
        let limits = (patience(0), patience(40 << 20), patience(1 << 40));
        assert_eq!(limits, (PER_FILE, Duration::from_secs(55), MOST));
    }
}
