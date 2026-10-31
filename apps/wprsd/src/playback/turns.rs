use crate::daemon::Daemon;
use crate::rotation::Act;
use crate::timing::nap;
use chrono::Utc;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Login {
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

async fn resumes(daemon: Arc<Daemon>) {
    let follow = async {
        let login = LoginProxy::new(&zbus::Connection::system().await?).await?;
        let mut sleeps = login.receive_prepare_for_sleep().await?;
        while let Some(signal) = sleeps.next().await {
            if signal.args().is_ok_and(|a| !a.start) {
                daemon.rotations.wake.notify_one();
                daemon.remote.poke.notify_one();
            }
        }
        zbus::Result::Ok(())
    };
    if let Err(e) = follow.await {
        tracing::info!("cannot follow suspend and resume: {e}");
    }
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub(super) const SETTLE: Duration = Duration::from_secs(3);

pub(super) fn settle(scanning: bool, since: Duration) -> bool {
    scanning && since < SETTLE
}

pub async fn run(daemon: Arc<Daemon>) {
    tokio::spawn(resumes(daemon.clone()));
    crate::clock::start(&daemon);
    let (mut serial, mut seen) = (daemon.changes.subscribe(), daemon.changes.serial());
    loop {
        let (print, worker, resolved) = (daemon.fingerprint(), daemon.clone(), Instant::now());
        let due = tokio::task::spawn_blocking(move || worker.show(Act::Check)).await.ok().flatten();
        let (due, mut pending) = (due.map(|at| at.saturating_mul(1000)), false);
        loop {
            let nap = nap(due, now_ms());
            tokio::select! {
                () = tokio::time::sleep(nap.unwrap_or_default()), if nap.is_some() => {
                    if due.is_some_and(|at| at <= now_ms()) {
                        break;
                    }
                }
                () = tokio::time::sleep_until(resolved + SETTLE), if pending => break,
                changed = serial.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    let (now, topics) = daemon.changes.since(seen);
                    seen = now;
                    if daemon.fingerprint() != print {
                        break;
                    }
                    if topics.iter().any(|t| t == "library") {
                        if !settle(daemon.jobs.scanning(), resolved.elapsed()) {
                            break;
                        }
                        pending = true;
                    }
                }
                () = daemon.rotations.wake.notified() => break,
            }
        }
    }
}
