use super::saved;
use crate::daemon::Daemon;
use crate::keyring::Keyring;
use std::future::pending;
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::sleep;
use wprs_core::model::JobKind;
use wprs_sync::Parser;

const EDITS: [&str; 3] = ["library", "playlists", "profiles"];
const PERIOD: Duration = Duration::from_secs(900);

pub fn start(daemon: &Arc<Daemon>, bus: zbus::Connection) {
    daemon.account.keyring.set(Keyring::server(bus)).ok();
    *daemon.saved() = saved::load(&daemon.sync_path());
    if daemon.saved().active() {
        daemon.account.wake.notify_one();
    }
    tokio::spawn(schedule(Arc::downgrade(daemon)));
    tokio::spawn(listen(Arc::downgrade(daemon)));
}

async fn edited(changes: &mut watch::Receiver<u64>, daemon: &Weak<Daemon>, serial: &mut u64) {
    loop {
        if changes.changed().await.is_err() {
            return pending().await;
        }
        let Some(daemon) = daemon.upgrade() else { return pending().await };
        let (now, topics) = daemon.changes.since(*serial);
        *serial = now;
        if topics.iter().any(|topic| EDITS.contains(&topic.as_str())) {
            return;
        }
    }
}

async fn schedule(daemon: Weak<Daemon>) {
    let Some(mut changes) = daemon.upgrade().map(|d| d.changes.subscribe()) else { return };
    let mut serial = *changes.borrow();
    loop {
        let Some(d) = daemon.upgrade() else { return };
        let period = if d.saved().active() { PERIOD } else { Duration::MAX };
        let wake = d.account.wake.notified();
        let edit = tokio::select! {
            () = sleep(period.min(Duration::from_secs(86_400 * 365))) => false,
            () = wake => false,
            () = edited(&mut changes, &daemon, &mut serial) => true,
        };
        sleep(Duration::from_secs(if edit { 5 } else { 0 })).await;
        if !d.saved().active() {
            continue;
        }
        if !d.jobs.find(JobKind::AccountSync, "").is_empty() {
            sleep(Duration::from_secs(5)).await;
            d.account.wake.notify_one();
        } else if !edit || d.worth_syncing().await {
            d.account_sync_job(false);
        }
    }
}

async fn listen(daemon: Weak<Daemon>) {
    let mut backoff = 30;
    loop {
        let Some(d) = daemon.upgrade() else { return };
        let (restart, active) = (d.account.restart.notified(), d.saved().active());
        let stream = match active {
            true => Some(d.events().await),
            false => None,
        };
        match stream {
            None => restart.await,
            Some(Ok(response)) => {
                backoff = 30;
                tokio::select! {
                    () = follow(response, &daemon) => sleep(Duration::from_secs(5)).await,
                    () = restart => {}
                }
            }
            Some(Err(e)) => {
                tracing::debug!("the server event stream is unavailable: {e}");
                tokio::select! {
                    () = sleep(Duration::from_secs(backoff)) => {}
                    () = restart => {}
                }
                backoff = (backoff * 2).min(900);
            }
        }
    }
}

async fn follow(mut response: reqwest::Response, daemon: &Weak<Daemon>) {
    let mut parser = Parser::default();
    while let Ok(Ok(Some(chunk))) = tokio::time::timeout(Duration::from_secs(60), response.chunk()).await {
        for (event, value) in parser.feed(&chunk) {
            let Some(d) = daemon.upgrade() else { return };
            let behind = value["seq"].as_i64().is_some_and(|seq| seq > d.saved().replica.seq);
            if (event == "seq" && behind) || event == "epoch" {
                d.account.wake.notify_one();
            }
        }
    }
}
