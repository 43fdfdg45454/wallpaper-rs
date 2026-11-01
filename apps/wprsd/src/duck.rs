use crate::compositor::lock;
use crate::daemon::{Daemon, settings};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::sync::watch;
use wprs_audio::Others;

const STEPS: u32 = 10;
const STEP: Duration = Duration::from_millis(50);

static GAIN: AtomicU64 = AtomicU64::new(0x3ff0_0000_0000_0000);
static WATCHER: Mutex<Option<(Others, watch::Sender<bool>)>> = Mutex::new(None);

pub(crate) fn gain() -> f64 {
    f64::from_bits(GAIN.load(Relaxed))
}

pub(crate) fn ours(pid: u32) -> bool {
    let own = std::process::id();
    let parent = |pid: u32| {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse::<u32>().ok()
    };
    std::iter::successors(Some(pid), |p| parent(*p).filter(|p| *p > 1)).take(5).any(|p| p == own)
}

pub(crate) fn follow(daemon: &Arc<Daemon>) {
    let wanted = daemon.state().config.playback.duck;
    let mut watcher = lock(&WATCHER);
    if wanted == watcher.is_some() {
        return;
    }
    let Some(_) = wanted.then_some(()) else {
        drop(watcher.take());
        GAIN.store(1f64.to_bits(), Relaxed);
        return daemon.compositor.set_settings(settings(&daemon.state().config));
    };
    let (tx, rx) = watch::channel(false);
    let told = tx.clone();
    match Others::watch(ours, move |playing| {
        told.send_replace(playing);
    }) {
        Ok(others) => {
            *watcher = Some((others, tx));
            tokio::spawn(fade(Arc::downgrade(daemon), rx));
        }
        Err(e) => tracing::warn!("cannot follow what other applications play: {e}"),
    }
}

async fn fade(daemon: Weak<Daemon>, mut playing: watch::Receiver<bool>) {
    while playing.changed().await.is_ok() {
        loop {
            let target = if *playing.borrow_and_update() { 0.0 } else { 1.0 };
            let now = gain();
            if (now - target).abs() < f64::EPSILON {
                break;
            }
            let step = (target - now).signum() / f64::from(STEPS);
            let next = if (target - now).abs() <= step.abs() { target } else { now + step };
            GAIN.store(next.to_bits(), Relaxed);
            let Some(daemon) = daemon.upgrade() else { return };
            daemon.compositor.set_settings(settings(&daemon.state().config));
            drop(daemon);
            tokio::time::sleep(STEP).await;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_daemon_and_its_helpers_are_its_own_and_others_are_not() {
        assert!(super::ours(std::process::id()));
        assert!(!super::ours(1) && !super::ours(u32::MAX));
    }
}
