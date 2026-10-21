use crate::daemon::{Daemon, OutputInfo, State, output_name, settings};
use serde_json::json;
use wprs_core::Config;

impl State {
    pub(crate) fn active(&self, info: &OutputInfo) -> bool {
        let p = &self.config.pause;
        !(self.user_paused
            || (p.when_covered && info.covered)
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
            change(&mut config).map_err(anyhow::Error::msg)?;
            config.save(&st.config_path)?;
            st.config = config;
            settings(&st.config)
        };
        self.compositor.set_settings(settings);
        self.apply_activity();
        Ok(())
    }

    pub fn set_option(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.update_config(|c| c.set_option(key, value))?;
        tracing::info!(key, value, "setting changed");
        Ok(())
    }

    pub fn set_audio(&self, volume: Option<f64>, muted: Option<bool>) -> anyhow::Result<()> {
        self.update_config(|c| {
            volume.map_or(Ok(()), |v| c.set_option("playback.volume", &v.to_string()))?;
            muted.map_or(Ok(()), |m| c.set_option("playback.muted", &m.to_string()))
        })
    }

    pub fn paused(&self) -> bool {
        self.state().user_paused
    }

    pub fn volume(&self) -> f64 {
        self.state().config.playback.sanitized().volume
    }

    pub fn muted(&self) -> bool {
        self.state().config.playback.muted
    }

    pub fn set_paused(&self, paused: bool) {
        self.state().user_paused = paused;
        tracing::info!(paused, "user pause changed");
        self.apply_activity();
    }

    pub fn set_on_battery(&self, on_battery: bool) {
        self.state().on_battery = on_battery;
        tracing::info!(on_battery, "power source changed");
        self.apply_activity();
    }

    pub fn set_locked(&self, locked: bool) {
        self.state().locked = locked;
        tracing::info!(locked, "screen lock changed");
        self.apply_activity();
    }

    pub fn set_covered(&self, output: &str, covered: bool) -> Result<(), String> {
        let output = output_name(output)?;
        let changed = (self.state().outputs.get_mut(&output))
            .is_some_and(|o| std::mem::replace(&mut o.covered, covered) != covered);
        if changed {
            tracing::debug!(output, covered, "output coverage changed");
            self.apply_activity();
        }
        Ok(())
    }

    pub fn state_json(&self) -> String {
        let status = self.compositor.status();
        let st = self.state();
        let outputs: serde_json::Map<_, _> = (st.outputs.iter())
            .map(|(name, info)| {
                let (r, s) = (st.config.resolve(name), status.get(name).cloned().unwrap_or_default());
                let (width, height) = info.size;
                let value = json!({ "width": width, "height": height, "image": r.image, "fit": r.fit.as_str(),
                    "kind": s.kind, "playing": s.playing, "active": st.active(info), "covered": info.covered,
                    "error": s.error });
                (name.clone(), value)
            })
            .collect();
        let playback = st.config.playback.sanitized();
        json!({ "version": env!("CARGO_PKG_VERSION"), "adapter": self.adapter, "config": st.config_path,
            "paused": st.user_paused, "on_battery": st.on_battery, "locked": st.locked, "volume": playback.volume,
            "muted": playback.muted, "options": st.config.options(), "outputs": outputs })
        .to_string()
    }
}
