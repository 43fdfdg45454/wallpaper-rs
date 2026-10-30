use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::Result;
use crate::profiles::auto;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wprs_core::model::ProfileAuto;

const HOLD: &str = "profile-hold.json";
const POWER_WAIT: Duration = Duration::from_secs(5);

static CHOICE: Mutex<()> = Mutex::new(());

type Conditions = (BTreeSet<String>, bool);
type Hold = (Option<String>, Conditions);

fn held(dir: &Path) -> Option<Hold> {
    serde_json::from_slice(&std::fs::read(dir.join(HOLD)).ok()?).ok()
}

fn holds(dir: &Path, active: Option<&str>, now: &Conditions) -> bool {
    let Some((profile, kept)) = held(dir) else { return false };
    let same = profile.as_deref() == active && kept == *now;
    if !same && !now.0.is_empty() {
        keep(dir, None)
            .unwrap_or_else(|e| tracing::warn!(error = %e, "cannot forget the profile chosen by hand"));
    }
    same
}

fn keep(dir: &Path, hold: Option<Hold>) -> std::result::Result<(), std::io::ErrorKind> {
    let path = dir.join(HOLD);
    let Some(hold) = hold else {
        return if path.exists() { std::fs::remove_file(path).map_err(|e| e.kind()) } else { Ok(()) };
    };
    let write = || {
        crate::store::private_dir(dir)?;
        let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
        file.write_all(&serde_json::to_vec(&hold)?)?;
        file.persist(path).map(drop).map_err(|e| e.error)
    };
    write().map_err(|e: std::io::Error| e.kind())
}

impl Daemon {
    pub fn hold(&self, manual: bool) {
        let profile = self.state().config.profile.clone();
        let hold = manual.then(|| (profile, conditions(self)));
        keep(&self.store.data, hold)
            .unwrap_or_else(|e| tracing::warn!(error = %e, "cannot remember the profile chosen by hand"));
    }
}

fn automatic(
    dir: &Path,
    active: Option<&str>,
    now: &Conditions,
    profiles: &[wprs_library::Profile],
) -> Option<String> {
    if holds(dir, active, now) {
        return None;
    }
    let (outputs, on_battery) = now;
    let met = |p: &&wprs_library::Profile| {
        let ProfileAuto { outputs: wanted, on_battery: battery } = auto(&p.auto);
        (wanted.is_some() || battery.is_some())
            && wanted.is_none_or(|w| w.into_iter().collect::<BTreeSet<_>>() == *outputs)
            && battery.is_none_or(|b| b == *on_battery)
    };
    profiles.iter().find(met).map(|p| p.uid.clone()).filter(|uid| active != Some(uid.as_str()))
}

pub(crate) fn choose(
    decide: impl FnOnce() -> Result<Option<String>>,
    apply: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    let _choice = lock(&CHOICE);
    decide()?.map_or(Ok(()), |profile| apply(&profile))
}

fn pick(daemon: &Daemon, now: &Conditions) -> Result<()> {
    let decide = || {
        let active = daemon.state().config.profile.clone();
        let profiles = daemon.store.read(|lib| lib.profiles())?;
        Ok(automatic(&daemon.store.data, active.as_deref(), now, &profiles))
    };
    choose(decide, |best| {
        tracing::info!("activating the profile whose conditions are met");
        daemon.activate(best).map(|()| daemon.hold(false))
    })
}

pub(crate) fn unhold(daemon: &Daemon) -> Result<()> {
    let decide = || {
        let held = !daemon.state().held.is_empty();
        Ok(daemon.state().config.profile.clone().filter(|_| held))
    };
    choose(decide, |profile| daemon.activate(profile))
}

fn conditions(daemon: &Daemon) -> Conditions {
    let st = daemon.state();
    (st.outputs.keys().filter(|o| !crate::drafts::virtual_output(o)).cloned().collect(), st.on_battery)
}

pub async fn follow(daemon: Arc<Daemon>, power: tokio::sync::oneshot::Receiver<()>) {
    let (mut serial, mut seen) = (daemon.changes.subscribe(), None::<Conditions>);
    let _ = tokio::time::timeout(POWER_WAIT, power).await;
    loop {
        let now = conditions(&daemon);
        if seen.as_ref() != Some(&now) {
            let outputs = seen.replace(now.clone()).is_none_or(|(outputs, _)| outputs != now.0);
            if outputs {
                tokio::time::sleep(Duration::from_secs(2)).await;
                if conditions(&daemon) != now {
                    continue;
                }
            }
            let worker = daemon.clone();
            match tokio::task::spawn_blocking(move || pick(&worker, &now)).await {
                Ok(Err(e)) => tracing::warn!("cannot activate a profile automatically: {e}"),
                Err(e) => tracing::warn!("cannot activate a profile automatically: {e}"),
                Ok(Ok(())) => {}
            }
        }
        if serial.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests;
