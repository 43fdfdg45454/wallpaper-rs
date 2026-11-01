use super::{EVERY, Stash};
use serde_json::Value as Json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Instant;

type Open = Vec<(PathBuf, Weak<Mutex<Stash>>)>;

static OPEN: Mutex<Open> = Mutex::new(Vec::new());
static BUILT: AtomicU64 = AtomicU64::new(0);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn drop_open(path: &Path) {
    lock(&OPEN).retain(|(p, w)| {
        if let Some(stash) = w.upgrade().filter(|_| p == path) {
            let mut s = lock(&stash);
            (s.forgotten, s.dirty) = (true, false);
        }
        p != path
    });
}

pub struct Keeper {
    shared: Arc<Mutex<Stash>>,
    output: String,
    writes: bool,
    known: u64,
    shown: u64,
    stale: bool,
    seq: u64,
    built: Option<Instant>,
    text: (u64, String),
}

impl Keeper {
    pub fn open(dir: &Path, package: &str, output: &str, writes: bool) -> Option<Keeper> {
        let path = super::file(dir, package)?;
        let mut open = lock(&OPEN);
        open.retain(|(_, w)| w.strong_count() > 0);
        let live = open.iter().find(|(p, _)| *p == path).and_then(|(_, w)| w.upgrade());
        let shared = match live {
            Some(s) => s,
            None => {
                let made = Arc::new(Mutex::new(Stash::open(dir, package)?));
                open.push((path, Arc::downgrade(&made)));
                made
            }
        };
        drop(open);
        let known = {
            let mut s = lock(&shared);
            s.touch(output);
            s.version()
        };
        let (output, text) = (output.to_owned(), (0, String::new()));
        let mut keeper =
            Keeper { shared, output, writes, known, shown: known, stale: true, seq: 0, built: None, text };
        keeper.refresh(Instant::now());
        Some(keeper)
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn scopes(&self) -> Json {
        lock(&self.shared).scopes(&self.output)
    }

    pub fn resend(&mut self) {
        (self.stale, self.seq) = (true, 0);
    }

    pub fn frame(&self) -> (u64, &str) {
        (self.text.0, &self.text.1)
    }

    pub fn take(&mut self, ops: &Json, now: Instant) {
        if !self.writes {
            return;
        }
        let mut s = lock(&self.shared);
        let before = s.version();
        let fine = s.merge(&self.output, ops);
        self.stale |= before != self.known || !fine;
        self.known = s.version();
        self.seq = ops.get("seq").and_then(Json::as_u64).unwrap_or(self.seq);
        settle(&mut s, now);
    }

    pub fn refresh(&mut self, now: Instant) {
        let mut s = lock(&self.shared);
        settle(&mut s, now);
        if s.version() != self.known {
            (self.known, self.stale) = (s.version(), true);
        }
        let slow = self.known != self.shown && self.built.is_none_or(|at| now.duration_since(at) >= EVERY);
        if !self.stale && !slow {
            return;
        }
        let mut scopes = s.scopes(&self.output);
        drop(s);
        scopes["seq"] = self.seq.into();
        self.text = (BUILT.fetch_add(1, Relaxed) + 1, scopes.to_string());
        (self.shown, self.stale, self.built) = (self.known, false, Some(now));
    }
}

fn settle(s: &mut Stash, now: Instant) {
    if s.due(now)
        && let Err(e) = s.flush(now, false)
    {
        tracing::warn!("cannot keep what a scene script stored: {e}");
    }
}
