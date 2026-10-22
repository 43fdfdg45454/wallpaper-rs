use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::library1::Library;
use crate::service::Service;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, sleep_until};
use wprs_core::dbus;
use zbus::object_server::SignalEmitter;

pub const TOPICS: [&str; 7] = ["state", "library", "playlists", "profiles", "providers", "repos", "jobs"];
const WINDOW: Duration = Duration::from_millis(250);
const KEPT: usize = 256;
const MAX_IDS: usize = 256;
const MAX_WAIT_MS: u32 = 20_000;

pub enum Signal {
    Changed(&'static str, Vec<String>),
    Progress(u64, u64, u64),
    Finished(u64, &'static str, String, String),
    Thumbnail(String, u32, String),
    Preview(String, String),
}

type Ids = Option<BTreeSet<String>>;

pub struct Changes {
    pub start: u64,
    serial: watch::Sender<u64>,
    log: Mutex<VecDeque<(u64, &'static str)>>,
    tx: mpsc::UnboundedSender<Signal>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<Signal>>>,
}

impl Changes {
    pub fn new() -> Self {
        let start = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_micros() as u64);
        let (tx, rx) = mpsc::unbounded_channel();
        Self { start, serial: watch::Sender::new(start), log: Mutex::default(), tx, rx: Mutex::new(Some(rx)) }
    }

    pub fn send(&self, signal: Signal) {
        let _ = self.tx.send(signal);
    }

    pub fn changed(&self, topic: &'static str, ids: Vec<String>) {
        self.send(Signal::Changed(topic, ids))
    }

    pub fn notifier(&self) -> impl Fn(Vec<String>) + Send + 'static {
        let tx = self.tx.clone();
        move |outputs| drop(tx.send(Signal::Changed("state", outputs)))
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.serial.subscribe()
    }

    pub fn serial(&self) -> u64 {
        *self.serial.borrow()
    }

    fn bump(&self, topic: &'static str) -> u64 {
        let mut log = lock(&self.log);
        let serial = self.serial() + 1;
        log.push_back((serial, topic));
        if log.len() > KEPT {
            log.pop_front();
        }
        self.serial.send_replace(serial);
        serial
    }

    pub fn since(&self, serial: u64) -> (u64, Vec<String>) {
        let (current, log) = (self.serial(), lock(&self.log));
        let complete =
            serial >= self.start && serial <= current && log.front().is_none_or(|(s, _)| *s <= serial + 1);
        let changed = |t: &&str| !complete || log.iter().any(|(s, topic)| *s > serial && topic == t);
        (current, TOPICS.into_iter().filter(changed).map(String::from).collect())
    }

    pub async fn wait(&self, serial: u64, timeout_ms: u32) -> (u64, Vec<String>) {
        let mut rx = self.serial.subscribe();
        if serial == *rx.borrow_and_update() {
            let timeout = Duration::from_millis(timeout_ms.min(MAX_WAIT_MS).into());
            if timeout.is_zero() || tokio::time::timeout(timeout, rx.changed()).await.is_err() {
                return (serial, Vec::new());
            }
        }
        self.since(serial)
    }
}

fn merge(pending: &mut Option<Ids>, ids: Vec<String>) {
    let merged = match pending.take() {
        None if ids.is_empty() => None,
        None => Some(ids.into_iter().collect()),
        Some(Some(mut set)) if !ids.is_empty() => {
            set.extend(ids);
            Some(set)
        }
        Some(_) => None,
    };
    *pending = Some(merged.filter(|set: &BTreeSet<String>| set.len() <= MAX_IDS));
}

pub async fn emit(connection: zbus::Connection, daemon: Arc<Daemon>) {
    let (Some(mut rx), Ok(emitter)) =
        (lock(&daemon.changes.rx).take(), SignalEmitter::new(&connection, dbus::OBJECT_PATH))
    else {
        return;
    };
    let mut windows: HashMap<&'static str, (Instant, Option<Ids>)> = HashMap::new();
    loop {
        let next = windows.values().map(|(until, _)| *until).min();
        let signal = tokio::select! {
            signal = rx.recv() => match signal { Some(signal) => signal, None => return },
            () = sleep_until(next.unwrap_or_else(Instant::now)), if next.is_some() => {
                let now = Instant::now();
                for (topic, (until, pending)) in windows.iter_mut().filter(|(_, (until, _))| *until <= now) {
                    if let Some(ids) = pending.take() {
                        let ids = ids.map(Vec::from_iter).unwrap_or_default();
                        changed(&daemon, &emitter, topic, ids).await;
                        *until = now + WINDOW;
                    }
                }
                windows.retain(|_, (until, _)| *until > now);
                continue;
            }
        };
        let sent = match signal {
            Signal::Changed(topic, ids) => {
                match windows.get_mut(topic) {
                    Some((_, pending)) => merge(pending, ids),
                    None => {
                        let mut first = None;
                        merge(&mut first, ids);
                        let ids = first.flatten().map(Vec::from_iter).unwrap_or_default();
                        changed(&daemon, &emitter, topic, ids).await;
                        windows.insert(topic, (Instant::now() + WINDOW, None));
                    }
                }
                Ok(())
            }
            Signal::Progress(job, done, total) => Service::job_progress(&emitter, job, done, total).await,
            Signal::Finished(job, state, name, message) => {
                Service::job_finished(&emitter, job, state, &name, &message).await
            }
            Signal::Thumbnail(uid, size, path) => Library::thumbnail_ready(&emitter, &uid, size, &path).await,
            Signal::Preview(spec, path) => crate::repos1::Repos::preview_ready(&emitter, &spec, &path).await,
        };
        sent.unwrap_or_else(|e| tracing::debug!("cannot emit a signal: {e}"));
    }
}

async fn changed(daemon: &Daemon, emitter: &SignalEmitter<'_>, topic: &'static str, ids: Vec<String>) {
    let serial = daemon.changes.bump(topic);
    let sent = Service::changed(emitter, topic, ids, serial).await;
    sent.unwrap_or_else(|e| tracing::debug!("cannot emit a change signal: {e}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_merge_into_a_union_or_everything() {
        let mut pending = None;
        merge(&mut pending, vec!["a".into()]);
        merge(&mut pending, vec!["b".into(), "a".into()]);
        assert_eq!(pending, Some(Some(BTreeSet::from(["a".into(), "b".into()]))));
        merge(&mut pending, Vec::new());
        merge(&mut pending, vec!["c".into()]);
        assert_eq!(pending, Some(None));
        let mut many = None;
        merge(&mut many, (0..=MAX_IDS).map(|n| n.to_string()).collect());
        assert_eq!(many, Some(None));
    }

    #[tokio::test]
    async fn waits_report_the_topics_changed_since_a_serial() {
        let changes = Changes::new();
        let start = changes.start;
        assert_eq!(changes.wait(start, 0).await, (start, vec![]));
        assert_eq!(changes.since(start - 1).1.len(), TOPICS.len());
        changes.bump("library");
        changes.bump("jobs");
        assert_eq!(changes.wait(start, 20).await, (start + 2, vec!["library".into(), "jobs".into()]));
        assert_eq!(changes.since(start + 1), (start + 2, vec!["jobs".into()]));
        assert_eq!(changes.since(start + 9).1.len(), TOPICS.len());
        for _ in 0..KEPT {
            changes.bump("state");
        }
        assert_eq!(changes.since(start).1.len(), TOPICS.len());
        assert_eq!(changes.since(start + 2).1, vec!["state".to_string()]);
    }
}
