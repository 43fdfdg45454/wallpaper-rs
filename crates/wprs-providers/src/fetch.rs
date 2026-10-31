use crate::stream::{CHUNK, Got, stream};
use crate::{AGENT, Result, local, redirect, refused};
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::{StatusCode, Url};
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use wprs_format::Fetch;

const WINDOW: Duration = Duration::from_secs(30);
const TICK: Duration = Duration::from_millis(100);
const MIN_BYTES: u64 = 8 << 10;

type Origins = Arc<Mutex<HashSet<String>>>;

pub struct HttpFetch {
    client: Client,
    cancel: Arc<AtomicBool>,
    window: Duration,
    origins: Origins,
}

impl HttpFetch {
    pub fn new(cancel: Arc<AtomicBool>) -> Result<Self> {
        Self::with_window(cancel, WINDOW)
    }

    pub fn with_window(cancel: Arc<AtomicBool>, window: Duration) -> Result<Self> {
        let origins = Origins::default();
        let known = origins.clone();
        let policy = Policy::custom(move |a| {
            if follows(&declared(&known), a.previous(), a.url()) { redirect(a, false) } else { a.stop() }
        });
        let builder = Client::builder().user_agent(AGENT).referer(false).redirect(policy);
        let client = builder.connect_timeout(Duration::from_secs(10)).timeout(window).build()?;
        Ok(Self { client, cancel, window, origins })
    }

    pub fn within(self, bases: &[String]) -> Self {
        let parsed = bases.iter().filter_map(|base| Url::parse(base).ok());
        declared(&self.origins).extend(parsed.map(|url| origin(&url)));
        self
    }

    fn live(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "transfer cancelled"));
        }
        Ok(())
    }
}

impl Fetch for HttpFetch {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        let mut bytes = Vec::new();
        Ok(self.copy(base, path, max, &mut bytes)?.then_some(bytes))
    }

    fn copy(&self, base: &str, path: &str, max: u64, out: &mut dyn Write) -> io::Result<bool> {
        self.live()?;
        if base.starts_with("file://") || !base.contains("://") {
            return self.local(base, path, max, out);
        }
        let url = Url::parse(&format!("{base}{path}"))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if !allowed(&url) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "plain http is only allowed to loopback hosts",
            ));
        }
        declared(&self.origins).insert(origin(&url));
        let (tx, rx) = mpsc::sync_channel(4);
        let request = self.client.get(url);
        thread::Builder::new().name("fetch".into()).spawn(move || stream(request, max, &tx))?;
        let (mut since, mut window) = (Instant::now(), 0);
        loop {
            self.live()?;
            match rx.recv_timeout(TICK) {
                Ok(Got::Status(StatusCode::NOT_FOUND | StatusCode::GONE)) => return Ok(false),
                Ok(Got::Status(status)) if !status.is_success() => {
                    return Err(io::Error::other(format!("HTTP status {status}")));
                }
                Ok(Got::Bytes(chunk)) => {
                    out.write_all(&chunk)?;
                    window += chunk.len() as u64;
                }
                Ok(Got::End) => return Ok(true),
                Ok(Got::Failed(e)) => return Err(e),
                Err(RecvTimeoutError::Disconnected) => return Err(io::Error::other("transfer ended")),
                Ok(Got::Status(_)) | Err(RecvTimeoutError::Timeout) => {}
            }
            if window >= MIN_BYTES {
                (since, window) = (Instant::now(), 0);
            } else if since.elapsed() >= self.window {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "transfer too slow"));
            }
        }
    }
}

impl HttpFetch {
    fn local(&self, base: &str, path: &str, max: u64, out: &mut dyn Write) -> io::Result<bool> {
        let root = base.strip_prefix("file://").unwrap_or(base);
        let mut file = match File::open(Path::new(root).join(path)) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            file => file?,
        };
        let (mut buf, mut left) = (vec![0; CHUNK], max.saturating_add(1));
        while left > 0 {
            self.live()?;
            let n = file.read(&mut buf[..left.min(CHUNK as u64) as usize])?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            left -= n as u64;
        }
        Ok(true)
    }
}

fn declared(origins: &Origins) -> MutexGuard<'_, HashSet<String>> {
    origins.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn origin(url: &Url) -> String {
    url.origin().ascii_serialization()
}

pub(crate) fn follows(known: &HashSet<String>, previous: &[Url], url: &Url) -> bool {
    known.contains(&origin(url)) && allowed(url) && !refused(previous, url, false)
}

fn allowed(url: &Url) -> bool {
    url.scheme() == "https" || url.scheme() == "http" && url.host_str().is_some_and(local)
}

pub(crate) fn kind(e: &reqwest::Error) -> io::ErrorKind {
    if e.is_timeout() { io::ErrorKind::TimedOut } else { io::ErrorKind::Other }
}

pub(crate) fn stalled(e: io::Error) -> io::Error {
    let inner = e.get_ref().and_then(|e| e.downcast_ref::<reqwest::Error>());
    match inner.map_or(e.kind(), kind) {
        io::ErrorKind::TimedOut => io::Error::new(io::ErrorKind::TimedOut, "transfer too slow"),
        _ => e,
    }
}
