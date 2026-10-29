use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::Result;
use crate::playlists1::library;
use crate::profiles::auto;
use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use wprs_core::model::ProfileAuto;

const HOLD: &str = "profile-hold.json";
const POWER_WAIT: Duration = Duration::from_secs(5);

static CHOICE: Mutex<()> = Mutex::new(());

type Conditions = (BTreeSet<String>, bool);
type Hold = (Option<String>, Conditions);

pub struct Profiles(pub Weak<Daemon>);

#[zbus::interface(name = "io.github.wallpaper_rs.Profiles1")]
impl Profiles {
    #[zbus(out_args("json"))]
    async fn list(&self) -> Result<String> {
        library(&self.0)?.run(|d| d.list_profiles()).await
    }

    #[zbus(out_args("json"))]
    async fn get(&self, profile: String) -> Result<String> {
        library(&self.0)?.run(move |d| d.get_profile(&profile)).await
    }

    #[zbus(out_args("uid"))]
    async fn save(&self, name: String, with_options: bool) -> Result<String> {
        library(&self.0)?.run(move |d| d.save_profile(&name, with_options)).await
    }

    async fn activate(&self, profile: String) -> Result<()> {
        let chosen =
            move |d: &Arc<Daemon>| choose(|| Ok(Some(profile)), |p| d.activate(p).map(|()| d.hold(true)));
        library(&self.0)?.render(chosen).await
    }

    async fn rename(&self, profile: String, name: String) -> Result<()> {
        library(&self.0)?
            .run(move |d| {
                d.edit_profile(&profile, |p| {
                    p.name = name;
                    Ok(())
                })
            })
            .await
    }

    async fn delete(&self, profile: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.delete_profile(&profile)).await
    }

    async fn set_auto(&self, profile: String, key: String, value: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.set_auto(&profile, &key, &value)).await
    }

    #[zbus(out_args("json"))]
    async fn list_presets(&self, item: String) -> Result<String> {
        library(&self.0)?.run(move |d| d.list_presets(&item)).await
    }

    #[zbus(out_args("uid"))]
    async fn save_preset(
        &self,
        item: String,
        name: String,
        values: HashMap<String, String>,
    ) -> Result<String> {
        library(&self.0)?.run(move |d| d.save_preset(&item, &name, &values)).await
    }

    async fn delete_preset(&self, preset: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.delete_preset(&preset)).await
    }
}

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

fn choose(
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
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_profile_chosen_by_hand_holds_until_its_conditions_change() {
        let dir = tempfile::tempdir().unwrap();
        let (docked, away) = ((BTreeSet::from(["DP-1".to_owned()]), false), (BTreeSet::new(), false));
        assert!(!holds(dir.path(), Some("P"), &docked));
        keep(dir.path(), Some((Some("P".into()), docked.clone()))).unwrap();
        let mode = std::fs::metadata(dir.path().join(HOLD)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(holds(dir.path(), Some("P"), &docked) && !holds(dir.path(), Some("P"), &away));
        assert!(holds(dir.path(), Some("P"), &docked) && !holds(dir.path(), None, &docked));
        assert!(!holds(dir.path(), Some("P"), &docked));
        keep(dir.path(), Some((Some("P".into()), docked.clone()))).unwrap();
        assert!(!holds(dir.path(), Some("P"), &(docked.0.clone(), true)));
        assert!(!holds(dir.path(), Some("P"), &docked));
        keep(dir.path(), None).unwrap();
    }

    #[test]
    fn a_failed_hold_reports_no_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(HOLD)).unwrap();
        let docked = (BTreeSet::from(["DP-1".to_owned()]), false);
        let failed = keep(dir.path(), Some((Some("P".into()), docked))).unwrap_err();
        let shown = failed.to_string();
        assert!(!shown.contains(&*dir.path().to_string_lossy()) && !shown.contains(".tmp-"), "{shown}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn an_automatic_choice_waits_for_a_choice_by_hand_and_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let docked = (BTreeSet::from(["DP-1".to_owned()]), false);
        let auto = serde_json::json!({"outputs": ["DP-1"]});
        let profile = |uid: &str, auto| wprs_library::Profile {
            uid: uid.into(),
            name: uid.into(),
            outputs: serde_json::json!({}),
            options: serde_json::json!({}),
            auto,
            priority: 0,
        };
        let profiles = vec![profile("DOCK", auto), profile("HAND", serde_json::json!({}))];
        assert_eq!(automatic(dir.path(), Some("HAND"), &docked, &profiles).as_deref(), Some("DOCK"));
        let (path, hold, (started, applying)) =
            (dir.path().to_owned(), docked.clone(), std::sync::mpsc::channel());
        let by_hand = std::thread::spawn(move || {
            let applied = |_: &str| {
                started.send(()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(200));
                keep(&path, Some((Some("HAND".into()), hold)))
                    .map_err(|e| crate::error::failed(format!("{e:?}")))
            };
            choose(|| Ok(Some("HAND".into())), applied)
        });
        applying.recv().unwrap();
        let mut chosen = None;
        let decide = || Ok(automatic(dir.path(), Some("HAND"), &docked, &profiles));
        let remember = |p: &str| {
            chosen = Some(p.to_owned());
            Ok(())
        };
        choose(decide, remember).unwrap();
        by_hand.join().unwrap().unwrap();
        assert_eq!(chosen, None, "an automatic choice replaced a choice made by hand meanwhile");
    }
}
