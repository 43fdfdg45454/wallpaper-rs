use crate::apply::target_output;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::items::reply;
use crate::jobs::now;
use crate::rotation::{Act, Playing, excluded};
use crate::timing::nap;
use chrono::Utc;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use wprs_core::Config;
use wprs_core::model::{HistoryEntry, HistoryReply, MODEL};

pub fn stop_sections(
    c: &mut Config,
    playing: &HashMap<String, Playing>,
    output: &str,
    only: Option<&str>,
) -> Vec<String> {
    let hit = |p: Option<&String>| p.is_some_and(|p| only.is_none_or(|u| u == p));
    let (mut stopped, mut fallback) = (Vec::new(), None);
    for (name, p) in playing.iter().filter(|(o, _)| output.is_empty() || *o == output) {
        if !hit(Some(&p.playlist.uid)) || c.resolve(name).playlist.as_ref() != Some(&p.playlist.uid) {
            continue;
        }
        let own = c.outputs.get(name).is_some_and(|s| s.playlist.is_some() || s.image.is_some());
        fallback = fallback.or(p.path.clone().filter(|_| !own));
        let section = c.outputs.entry(name.clone()).or_default();
        (section.image, section.playlist) = (p.path.clone(), None);
        stopped.push(p.playlist.uid.clone());
    }
    let sections =
        std::iter::once(("", &mut c.default)).chain(c.outputs.iter_mut().map(|(n, s)| (n.as_str(), s)));
    for (name, section) in
        sections.filter(|(n, s)| hit(s.playlist.as_ref()) && (output.is_empty() || *n == output))
    {
        stopped.extend(section.playlist.take());
        if name.is_empty() {
            section.image = fallback.clone();
        }
    }
    stopped.sort();
    stopped.dedup();
    stopped
}

impl Daemon {
    pub fn start(&self, key: &str, output: &str) -> Result<()> {
        let (output, revoked, on_battery) = (target_output(output)?, self.revoked(), self.state().on_battery);
        let resolved =
            self.store.read(|lib| lib.resolve(key, now(), on_battery, &excluded(lib, &revoked)))?;
        if resolved.uids.is_empty() {
            return Err(failed("the playlist has no items to show"));
        }
        let uid = resolved.playlist.id;
        self.update_config(|c| {
            c.set_playlist(&output, uid.clone());
            Ok(())
        })?;
        self.show(Act::Start(&uid, &output));
        self.changes.changed("playlists", vec![uid]);
        Ok(())
    }

    pub fn stop(&self, output: &str) -> Result<()> {
        let (output, playing) = (target_output(output)?, self.state().playing.clone());
        let mut stopped = Vec::new();
        self.update_config(|c| {
            stopped = stop_sections(c, &playing, &output, None);
            Ok(())
        })?;
        self.configure_all();
        self.changes.changed("playlists", stopped);
        Ok(())
    }

    pub fn step(&self, output: &str, forward: bool) -> Result<()> {
        let output = target_output(output)?;
        let assigned = self.state().assigned().into_iter().map(|(o, ..)| o);
        let outputs: Vec<String> = assigned.filter(|o| output.is_empty() || *o == output).collect();
        if outputs.is_empty() && !output.is_empty() {
            return Err(not_found(format!("{output} has no playlist")));
        }
        self.show(Act::Step(&outputs, forward));
        Ok(())
    }

    pub fn history(&self, output: &str, limit: u32) -> Result<String> {
        let (output, serial) = (target_output(output)?, self.changes.serial());
        if output.is_empty() || !(1..=1000).contains(&limit) {
            return Err(invalid("give a connector name and a limit of 1 to 1000"));
        }
        let used = self.store.read(|lib| {
            let entries = lib.history(&output, limit)?.into_iter();
            Ok::<_, Error>(
                entries.map(|u| (u.at, u.item.and_then(|uid| lib.item(&uid).ok()))).collect::<Vec<_>>(),
            )
        })?;
        let entry = |(at, item): (i64, Option<_>)| {
            Ok(HistoryEntry { at, item: item.map(|i| self.summary(i)).transpose()? })
        };
        let entries = used.into_iter().map(entry).collect::<Result<_>>()?;
        reply(HistoryReply { model: MODEL, serial, output, entries })
    }
}

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

pub async fn run(daemon: Arc<Daemon>) {
    tokio::spawn(resumes(daemon.clone()));
    let (mut serial, mut seen) = (daemon.changes.subscribe(), daemon.changes.serial());
    loop {
        let (print, worker) = (daemon.fingerprint(), daemon.clone());
        let due = tokio::task::spawn_blocking(move || worker.show(Act::Check)).await.ok().flatten();
        let due = due.map(|at| at.saturating_mul(1000));
        loop {
            let nap = nap(due, now_ms());
            tokio::select! {
                () = tokio::time::sleep(nap.unwrap_or_default()), if nap.is_some() => {
                    if due.is_some_and(|at| at <= now_ms()) {
                        break;
                    }
                }
                changed = serial.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    let (now, topics) = daemon.changes.since(seen);
                    seen = now;
                    if topics.iter().any(|t| t == "library") || daemon.fingerprint() != print {
                        break;
                    }
                }
                () = daemon.rotations.wake.notified() => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wprs_core::model::OutputPlaylist;

    #[test]
    fn stopping_keeps_the_item_shown() {
        let shown = |uid: &str, path: &str| {
            let playlist =
                OutputPlaylist { uid: uid.into(), name: uid.into(), position: 0, count: 1, next_at: None };
            Playing { playlist, item: None, path: Some(path.into()), error: None, transition: None }
        };
        let mut config = Config::default();
        config.set_playlist("", "A".into());
        config.set_playlist("DP-2", "B".into());
        let playing =
            HashMap::from([("DP-1".into(), shown("A", "/w/a.png")), ("DP-2".into(), shown("B", "/w/b.png"))]);
        let mut one = config.clone();
        assert_eq!(stop_sections(&mut one, &playing, "DP-2", None), ["B"]);
        assert_eq!(
            (one.resolve("DP-2").image, one.resolve("DP-1").playlist),
            (Some("/w/b.png".into()), Some("A".into()))
        );
        assert_eq!(stop_sections(&mut config, &playing, "", Some("A")), ["A"]);
        assert_eq!((&config.default.image, &config.default.playlist), (&Some("/w/a.png".into()), &None));
        assert_eq!(config.resolve("DP-2").playlist.as_deref(), Some("B"));
    }
}
