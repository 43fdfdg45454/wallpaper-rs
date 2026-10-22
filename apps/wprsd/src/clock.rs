use crate::daemon::Daemon;
use futures_util::StreamExt;
use rustix::io::Errno;
use rustix::time::{ClockId, Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec};
use std::os::fd::OwnedFd;
use std::sync::{Arc, Weak};
use std::time::Duration;
use zbus::fdo::PropertiesProxy;
use zbus::proxy::CacheProperties;

const TIMEDATE: &str = "org.freedesktop.timedate1";
const ZONE_SETTLES: Duration = Duration::from_millis(1100);
const YEAR: i64 = 366 * 86_400;

fn arm(timer: &OwnedFd) -> rustix::io::Result<()> {
    let far = Timespec { tv_sec: rustix::time::clock_gettime(ClockId::Realtime).tv_sec + YEAR, tv_nsec: 0 };
    let value = Itimerspec { it_interval: Timespec { tv_sec: 0, tv_nsec: 0 }, it_value: far };
    let flags = TimerfdTimerFlags::ABSTIME | TimerfdTimerFlags::CANCEL_ON_SET;
    rustix::time::timerfd_settime(timer, flags, &value).map(drop)
}

fn timer() -> rustix::io::Result<OwnedFd> {
    let timer = rustix::time::timerfd_create(TimerfdClockId::Realtime, TimerfdFlags::CLOEXEC)?;
    arm(&timer).map(|()| timer)
}

fn follow_jumps(daemon: Weak<Daemon>) -> std::io::Result<()> {
    let timer = timer()?;
    let watch = move || {
        loop {
            match rustix::io::read(&timer, &mut [0; 8]) {
                Err(Errno::INTR) => {}
                Err(Errno::CANCELED) if arm(&timer).is_ok() => match daemon.upgrade() {
                    Some(daemon) => daemon.rotations.retime(),
                    None => return,
                },
                Ok(_) if arm(&timer).is_ok() => {}
                _ => return,
            }
        }
    };
    std::thread::Builder::new().name("wprs-clock".into()).spawn(watch).map(drop)
}

async fn follow_zone(daemon: Arc<Daemon>) -> zbus::Result<()> {
    let system = zbus::Connection::system().await?;
    let proxy = PropertiesProxy::builder(&system).destination(TIMEDATE).path("/org/freedesktop/timedate1");
    let mut changes =
        proxy.cache_properties(CacheProperties::No).build().await?.receive_properties_changed().await?;
    while let Some(change) = changes.next().await {
        let zone = |a: zbus::fdo::PropertiesChangedArgs<'_>| {
            a.changed_properties().contains_key("Timezone")
                || a.invalidated_properties().contains(&"Timezone")
        };
        if change.args().is_ok_and(zone) {
            tokio::time::sleep(ZONE_SETTLES).await;
            daemon.rotations.retime();
        }
    }
    Ok(())
}

pub fn start(daemon: &Arc<Daemon>) {
    if let Err(e) = follow_jumps(Arc::downgrade(daemon)) {
        tracing::info!("cannot follow changes of the clock: {e}");
    }
    let daemon = daemon.clone();
    let sandboxed = crate::flatpak::app_id().is_some();
    tokio::spawn(async move {
        let followed = match sandboxed {
            true => crate::flatpak::follow_zone(Arc::downgrade(&daemon)).map_err(zbus::Error::from),
            false => follow_zone(daemon).await,
        };
        if let Err(e) = followed {
            tracing::info!("cannot follow time zone changes: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_timer_waits_for_a_change() {
        let timer = timer().unwrap();
        rustix::io::ioctl_fionbio(&timer, true).unwrap();
        assert_eq!(rustix::io::read(&timer, &mut [0; 8]), Err(Errno::AGAIN));
    }
}
