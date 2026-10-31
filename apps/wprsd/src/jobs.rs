use crate::changes::Signal;
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, not_found};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, SemaphorePermit};
use wprs_core::model::{Job, JobKind, JobState};

mod control;

const KEPT_FOR: i64 = 600;
const KEPT_MAX: usize = 64;
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

struct Entry {
    job: Job,
    cancel: Arc<AtomicBool>,
}

pub struct Jobs {
    next: AtomicU64,
    list: Mutex<Vec<Entry>>,
    library: Semaphore,
    repos: Semaphore,
    downloads: Semaphore,
    account: Semaphore,
    linking: Semaphore,
    uploads: Semaphore,
    runtime: tokio::runtime::Handle,
}

pub struct Ctl {
    id: u64,
    cancel: Arc<AtomicBool>,
    shown: Mutex<Option<Instant>>,
    daemon: Arc<Daemon>,
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn active(job: &Job) -> bool {
    matches!(job.state, JobState::Queued | JobState::Running)
}

fn joins(job: &Job, kind: JobKind, target: &str) -> bool {
    let fresh = match kind {
        JobKind::Import => false,
        JobKind::Scan => job.state == JobState::Queued,
        _ => active(job),
    };
    fresh && job.kind == kind && job.target == target
}

impl Jobs {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Handle::current();
        let (library, repos, downloads) = (Semaphore::new(1), Semaphore::new(1), Semaphore::new(2));
        let (account, linking, uploads) = (Semaphore::new(1), Semaphore::new(1), Semaphore::new(1));
        let list = Mutex::default();
        Self { next: AtomicU64::new(1), list, library, repos, downloads, account, linking, uploads, runtime }
    }

    fn queue(&self, kind: JobKind) -> &Semaphore {
        match kind {
            JobKind::ProviderSync => &self.downloads,
            JobKind::RepoAdd | JobKind::RepoRefresh | JobKind::Install => &self.repos,
            JobKind::Account | JobKind::AccountSync => &self.account,
            JobKind::Upload => &self.uploads,
            JobKind::Link => &self.linking,
            _ => &self.library,
        }
    }

    pub fn wait_turn(&self, kind: JobKind) -> Option<SemaphorePermit<'_>> {
        self.runtime.block_on(self.queue(kind).acquire()).ok()
    }

    fn with<T>(&self, id: u64, change: impl FnOnce(&mut Entry) -> T) -> Option<T> {
        lock(&self.list).iter_mut().find(|e| e.job.id == id).map(change)
    }

    pub fn list(&self) -> Vec<Job> {
        let (mut list, now) = (lock(&self.list), now());
        list.retain(|e| e.job.finished_at.is_none_or(|at| now - at < KEPT_FOR));
        let mut extra = list.iter().filter(|e| !active(&e.job)).count().saturating_sub(KEPT_MAX);
        list.retain(|e| {
            let dropped = !active(&e.job) && extra > 0;
            extra -= usize::from(dropped);
            !dropped
        });
        list.iter().map(|e| e.job.clone()).collect()
    }

    pub fn get(&self, id: u64) -> Result<Job> {
        let job = self.list().into_iter().find(|job| job.id == id);
        job.ok_or_else(|| not_found(format!("job {id} not found")))
    }

    pub fn active(&self) -> u32 {
        lock(&self.list).iter().filter(|e| active(&e.job)).count() as u32
    }

    pub fn find(&self, kind: JobKind, target: &str) -> Vec<u64> {
        let matches = |e: &&Entry| e.job.kind == kind && e.job.target == target && active(&e.job);
        lock(&self.list).iter().filter(matches).map(|e| e.job.id).collect()
    }

    pub fn scanning(&self) -> bool {
        let library =
            |kind| matches!(kind, JobKind::Scan | JobKind::Import | JobKind::Analyze | JobKind::ProviderSync);
        lock(&self.list).iter().any(|e| library(e.job.kind) && active(&e.job))
    }

    pub fn wait(&self, ids: &[u64]) {
        while lock(&self.list).iter().any(|e| ids.contains(&e.job.id) && active(&e.job)) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Ctl {
    pub fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) { Err(Error::Cancelled) } else { Ok(()) }
    }

    pub fn flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn progress(&self, done: u64, total: u64) {
        self.daemon.jobs.with(self.id, |e| (e.job.done, e.job.total) = (done, total));
        let mut shown = lock(&self.shown);
        if shown.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY) {
            *shown = Some(Instant::now());
            self.daemon.changes.send(Signal::Progress(self.id, done, total));
        }
    }
}

#[cfg(test)]
mod tests;
