use crate::daemon::Daemon;
use crate::error::Result;
use crate::playlists1::library;
use crate::profiles::auto;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Weak};
use std::time::Duration;
use wprs_core::model::ProfileAuto;

type Conditions = (BTreeSet<String>, bool);

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
        library(&self.0)?.run(move |d| d.activate(&profile)).await
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

fn pick(daemon: &Daemon, (outputs, on_battery): &Conditions) -> Result<()> {
    let holds = |p: &wprs_library::Profile| {
        let ProfileAuto { outputs: wanted, on_battery: battery } = auto(&p.auto);
        (wanted.is_some() || battery.is_some())
            && wanted.is_none_or(|w| w.into_iter().collect::<BTreeSet<_>>() == *outputs)
            && battery.is_none_or(|b| b == *on_battery)
    };
    let Some(best) = daemon.store.read(|lib| lib.profiles())?.into_iter().find(holds) else {
        return Ok(());
    };
    if daemon.state().config.profile.as_ref() == Some(&best.uid) {
        return Ok(());
    }
    tracing::info!("activating the profile whose conditions are met");
    tracing::debug!(profile = best.name, "automatic activation");
    daemon.activate(&best.uid)
}

fn conditions(daemon: &Daemon) -> Conditions {
    let st = daemon.state();
    (st.outputs.keys().cloned().collect(), st.on_battery)
}

pub async fn follow(daemon: Arc<Daemon>) {
    let (mut serial, mut seen) = (daemon.changes.subscribe(), None::<Conditions>);
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
