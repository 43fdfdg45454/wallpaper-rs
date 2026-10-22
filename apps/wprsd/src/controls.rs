use crate::daemon::{Daemon, State, settings};
use crate::streams::OutputInfo;
use std::path::Path;
use wprs_core::{Config, Fit};
use zbus::fdo::Error::InvalidArgs;

impl State {
    pub(crate) fn active(&self, info: &OutputInfo) -> bool {
        let p = &self.config.pause;
        !(self.user_paused
            || (p.when_covered && info.covered())
            || (p.on_battery && self.on_battery)
            || (p.when_locked && self.locked))
    }
}

impl Daemon {
    pub(crate) fn update_config(
        &self,
        change: impl FnOnce(&mut Config) -> Result<(), String>,
    ) -> anyhow::Result<()> {
        let settings = {
            let mut st = self.state();
            let mut config = st.config.clone();
            change(&mut config).map_err(InvalidArgs)?;
            if st.config_error.is_some() && st.config_path.exists() {
                let backup = Config::backup_broken(&st.config_path)?;
                tracing::warn!("kept a copy of the broken configuration");
                tracing::debug!(backup = %backup.display(), "broken configuration copy");
            }
            config.save(&st.config_path)?;
            (st.config, st.config_error) = (config, None);
            settings(&st.config)
        };
        self.compositor.set_settings(settings);
        self.apply_activity();
        self.refresh_items();
        self.publish();
        Ok(())
    }

    pub fn set_option(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.update_config(|c| c.set_option(key, value))?;
        tracing::info!(key, value, "setting changed");
        if key.starts_with("library.") {
            self.watch_sources();
        }
        Ok(())
    }

    pub fn set_audio(&self, volume: Option<f64>, muted: Option<bool>) -> anyhow::Result<()> {
        self.update_config(|c| {
            volume.map_or(Ok(()), |v| c.set_option("playback.volume", &v.to_string()))?;
            muted.map_or(Ok(()), |m| c.set_option("playback.muted", &m.to_string()))
        })
    }

    pub fn change_wallpaper(&self, path: &Path, output: &str, fit: &str) -> anyhow::Result<()> {
        let fit = Some(fit.trim()).filter(|f| !f.is_empty()).map(str::parse::<Fit>).transpose();
        let fit = fit.map_err(|e| InvalidArgs(e.to_string()))?;
        match (path.as_os_str().is_empty(), output.trim(), fit) {
            (true, _, None) => Err(InvalidArgs("give a wallpaper, a fit or both".into()).into()),
            (true, output, Some(fit)) => self.change_output(output, |c| {
                c.set_fit(output, fit);
                Ok(())
            }),
            (false, output, fit) => self.set_wallpaper(path, output, fit),
        }
    }

    pub fn set_background(&self, color: &str, output: &str) -> anyhow::Result<()> {
        let output = output.trim();
        self.change_output(output, |c| c.set_background(output, color))
    }

    pub(crate) fn change_output(
        &self,
        output: &str,
        change: impl FnOnce(&mut Config) -> Result<(), String>,
    ) -> anyhow::Result<()> {
        self.update_config(change)?;
        match output.is_empty() {
            true => self.configure_all(),
            false => drop(self.configure_output(output)),
        }
        Ok(())
    }

    pub fn set_paused(&self, paused: bool) -> anyhow::Result<()> {
        self.state().user_paused = paused;
        tracing::debug!(paused, "user pause changed");
        self.apply_activity();
        self.publish();
        Ok(())
    }

    pub fn set_on_battery(&self, on_battery: bool) {
        self.state().on_battery = on_battery;
        tracing::debug!(on_battery, "power source changed");
        self.apply_activity();
        self.changes.changed("state", Vec::new());
    }

    pub fn set_locked(&self, locked: bool) {
        self.state().locked = locked;
        tracing::debug!(locked, "screen lock changed");
        self.apply_activity();
        self.changes.changed("state", Vec::new());
    }

    pub(crate) fn apply_activity(&self) {
        self.activity(&self.state());
    }

    pub(crate) fn activity(&self, st: &State) {
        st.outputs.iter().for_each(|(name, info)| self.compositor.set_active(name, st.active(info)));
    }
}
