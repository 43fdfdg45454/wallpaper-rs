use crate::{AGENT, Result, local, redirect};
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use wprs_format::{Fetch, FileFetch};

const WINDOW: Duration = Duration::from_secs(30);
const MIN_BYTES: u64 = 8 << 10;

pub struct HttpFetch {
    client: Client,
    cancel: Arc<AtomicBool>,
    window: Duration,
}

impl HttpFetch {
    pub fn new(cancel: Arc<AtomicBool>) -> Result<Self> {
        Self::with_window(cancel, WINDOW)
    }

    pub fn with_window(cancel: Arc<AtomicBool>, window: Duration) -> Result<Self> {
        let policy = Policy::custom(|a| if allowed(a.url()) { redirect(a, false) } else { a.stop() });
        let builder = Client::builder().user_agent(AGENT).referer(false).redirect(policy);
        let client = builder.connect_timeout(Duration::from_secs(10)).timeout(window).build()?;
        Ok(Self { client, cancel, window })
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
        if base.starts_with("file://") || !base.contains("://") {
            return FileFetch.copy(base, path, max, out);
        }
        self.live()?;
        let url = Url::parse(&format!("{base}{path}"))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if !allowed(&url) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "plain http is only allowed to loopback hosts",
            ));
        }
        let response = self.client.get(url).send();
        let mut response = response.map_err(|e| io::Error::new(kind(&e), e.without_url()))?;
        match response.status() {
            StatusCode::NOT_FOUND | StatusCode::GONE => return Ok(false),
            status if !status.is_success() => return Err(io::Error::other(format!("HTTP status {status}"))),
            _ => {}
        }
        let (mut buf, mut left) = (vec![0; 64 << 10], max.saturating_add(1));
        let (mut since, mut window) = (Instant::now(), 0);
        while left > 0 {
            self.live()?;
            let n = response.read(&mut buf[..left.min(64 << 10) as usize]).map_err(stalled)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            (left, window) = (left - n as u64, window + n as u64);
            if window >= MIN_BYTES {
                (since, window) = (Instant::now(), 0);
            } else if since.elapsed() >= self.window {
                return Err(stalled(io::ErrorKind::TimedOut.into()));
            }
        }
        Ok(true)
    }
}

fn allowed(url: &Url) -> bool {
    url.scheme() == "https" || url.scheme() == "http" && url.host_str().is_some_and(local)
}

fn kind(e: &reqwest::Error) -> io::ErrorKind {
    if e.is_timeout() { io::ErrorKind::TimedOut } else { io::ErrorKind::Other }
}

fn stalled(e: io::Error) -> io::Error {
    let inner = e.get_ref().and_then(|e| e.downcast_ref::<reqwest::Error>());
    match inner.map_or(e.kind(), kind) {
        io::ErrorKind::TimedOut => io::Error::new(io::ErrorKind::TimedOut, "transfer too slow"),
        _ => e,
    }
}
