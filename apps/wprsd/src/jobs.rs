use crate::changes::Signal;
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, not_found};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use wprs_core::model::{Job, JobError, JobKind, JobState};

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

impl Jobs {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Handle::current();
        Self { next: AtomicU64::new(1), list: Mutex::default(), library: Semaphore::new(1), runtime }
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
}

impl Ctl {
    pub fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) { Err(Error::Cancelled) } else { Ok(()) }
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

impl Daemon {
    pub fn start_job<F>(self: &Arc<Self>, kind: JobKind, target: &str, work: F) -> u64
    where
        F: FnOnce(&Arc<Daemon>, &Ctl) -> Result<Value> + Send + 'static,
    {
        let id = {
            let mut list = lock(&self.jobs.list);
            let same = |e: &&Entry| e.job.kind == kind && e.job.target == target && active(&e.job);
            if let Some(e) = list.iter().find(same).filter(|_| kind != JobKind::Import) {
                return e.job.id;
            }
            let id = self.jobs.next.fetch_add(1, Ordering::Relaxed);
            let (state, target, started_at) = (JobState::Queued, target.to_owned(), now());
            let (finished_at, error, output) = (None, None, None);
            let job =
                Job { id, kind, target, state, done: 0, total: 0, started_at, finished_at, error, output };
            list.push(Entry { job, cancel: Arc::default() });
            id
        };
        self.jobs_changed(id);
        let daemon = self.clone();
        self.jobs.runtime.spawn(async move {
            let _turn = daemon.jobs.library.acquire().await;
            let start = |e: &mut Entry| {
                let queued = e.job.state == JobState::Queued;
                e.job.state = if queued { JobState::Running } else { e.job.state };
                queued.then(|| e.cancel.clone())
            };
            let Some(cancel) = daemon.jobs.with(id, start).flatten() else { return };
            let ctl = Ctl { id, cancel, shown: Mutex::default(), daemon: daemon.clone() };
            let done = tokio::task::spawn_blocking(move || {
                let result = work(&ctl.daemon, &ctl);
                if ctl.check().is_err() { Err(Error::Cancelled) } else { result }
            });
            let result = done.await.unwrap_or_else(|e| Err(failed(format!("internal error: {e}"))));
            daemon.finish_job(id, result);
        });
        id
    }

    fn finish_job(&self, id: u64, result: Result<Value>) {
        let (state, output, error) = match result {
            Ok(output) => (JobState::Done, Some(output), None),
            Err(Error::Cancelled) => (JobState::Cancelled, None, None),
            Err(e) => {
                let (name, message) = e.parts();
                tracing::warn!(job = id, error = name, "job failed");
                tracing::debug!(job = id, "job failure: {message}");
                (JobState::Failed, None, Some(JobError { name, message }))
            }
        };
        let (name, message) =
            error.as_ref().map_or_else(Default::default, |e| (e.name.clone(), e.message.clone()));
        let finish = |e: &mut Entry| {
            (e.job.state, e.job.finished_at, e.job.error, e.job.output) = (state, Some(now()), error, output)
        };
        self.jobs.with(id, finish);
        let (state, message) = match state {
            JobState::Done => ("done", message),
            JobState::Cancelled => ("cancelled", "cancelled".into()),
            _ => ("failed", message),
        };
        self.changes.send(Signal::Finished(id, state, name, message));
        self.jobs_changed(id);
    }

    fn jobs_changed(&self, id: u64) {
        self.changes.changed("jobs", vec![id.to_string()]);
        self.changes.changed("state", Vec::new());
    }

    pub fn cancel_job(&self, id: u64) -> Result<()> {
        if id == 0 || id >= self.jobs.next.load(Ordering::Relaxed) {
            return Err(not_found(format!("job {id} not found")));
        }
        let cancel = |e: &mut Entry| {
            e.cancel.store(true, Ordering::Relaxed);
            let queued = e.job.state == JobState::Queued;
            e.job.state = if queued { JobState::Cancelled } else { e.job.state };
            queued
        };
        if self.jobs.with(id, cancel).unwrap_or(false) {
            self.finish_job(id, Err(Error::Cancelled));
        }
        Ok(())
    }
}
