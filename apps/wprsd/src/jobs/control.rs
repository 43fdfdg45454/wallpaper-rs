use super::{Ctl, Entry, joins, now};
use crate::changes::Signal;
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, not_found};
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use wprs_core::model::{Job, JobError, JobKind, JobState};

impl Daemon {
    pub fn start_job<F>(self: &Arc<Self>, kind: JobKind, target: &str, work: F) -> u64
    where
        F: FnOnce(&Arc<Daemon>, &Ctl) -> Result<Value> + Send + 'static,
    {
        let id = {
            let mut list = lock(&self.jobs.list);
            if let Some(e) = list.iter().find(|e| joins(&e.job, kind, target)) {
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
            let _turn = daemon.jobs.queue(kind).acquire().await;
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
