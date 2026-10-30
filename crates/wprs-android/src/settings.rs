use crate::account::NETWORK;
use crate::{Error, power, store};
use std::fs;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use wprs_core::settings::fps::DISPLAY;
use wprs_core::{Config, Fit};

pub(crate) const COLORS: &str = "system_colors";
pub(crate) const BACKEND: &str = "backend";
const CHARGER: &str = "charger_fps";
const FIT: &str = "fit";
const BATTERY: &str = "playback.max_fps";
const CHOICES: [(&str, &[&str]); 4] = [
    (COLORS, &["chosen", "always", "never"]),
    (CHARGER, &["60", "30"]),
    (BACKEND, &["auto", "vulkan", "gl"]),
    (NETWORK, &["unmetered", "any"]),
];
const SHARED: &str = "config.toml";
const OWN: &str = "settings.toml";

static CONFIG: Mutex<Option<Config>> = Mutex::new(None);

pub(crate) struct Look {
    pub(crate) fit: Fit,
    pub(crate) background: [u8; 3],
    pub(crate) fade: Duration,
}

pub(crate) fn prepare() -> Result<(), Error> {
    let mut own = table()?;
    let (fit, sound) = (own.remove("video_fit"), own.remove("video_sound"));
    if fit.is_some() || sound.is_some() {
        update(|config| {
            if fit.as_ref().and_then(toml::Value::as_str) == Some("stretch") {
                config.set_fit("", Fit::Stretch);
            }
            config.playback.muted = sound.as_ref().and_then(toml::Value::as_str) != Some("on");
            Ok(())
        })?;
        store::write(&store::dir()?.join(OWN), own.to_string().as_bytes())?;
    }
    caps();
    Ok(())
}

fn cached() -> Result<MutexGuard<'static, Option<Config>>, Error> {
    let mut cached = CONFIG.lock().unwrap_or_else(PoisonError::into_inner);
    if cached.is_none() {
        *cached = Some(Config::load(&store::dir()?.join(SHARED))?);
    }
    Ok(cached)
}

#[cfg(test)]
pub(crate) fn reload() {
    *CONFIG.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

fn loaded() -> Result<Config, Error> {
    Ok(cached()?.clone().unwrap_or_default())
}

pub(crate) fn config() -> Config {
    loaded().unwrap_or_default()
}

pub(crate) fn downloads() -> bool {
    loaded().is_ok_and(|config| config.models.allow_download)
}

pub(crate) fn update(change: impl FnOnce(&mut Config) -> Result<(), String>) -> Result<(), Error> {
    {
        let mut cached = cached()?;
        let mut config = cached.clone().unwrap_or_default();
        change(&mut config).map_err(Error::Setting)?;
        config.save(&store::dir()?.join(SHARED))?;
        *cached = Some(config);
    }
    store::changed();
    caps();
    Ok(())
}

fn table() -> Result<toml::Table, Error> {
    let text = fs::read_to_string(store::dir()?.join(OWN)).unwrap_or_default();
    Ok(text.parse().unwrap_or_default())
}

pub(crate) fn setting(key: &str, value: Option<&str>) -> Result<String, Error> {
    if let Some((_, allowed)) = CHOICES.iter().find(|(k, _)| *k == key) {
        let mut own = table()?;
        if let Some(value) = value {
            let valid = allowed.contains(&value).then_some(value);
            own.insert(key.into(), valid.ok_or_else(|| Error::Setting(format!("{key} = {value}")))?.into());
            store::write(&store::dir()?.join(OWN), own.to_string().as_bytes())?;
            store::changed();
            caps();
        }
        let saved = own.get(key).and_then(toml::Value::as_str).filter(|v| allowed.contains(v));
        return Ok(saved.unwrap_or(allowed[0]).to_owned());
    }
    if let Some(value) = value {
        update(|config| match key {
            FIT => {
                config.set_fit("", value.parse().map_err(|e: wprs_core::fit::UnknownFit| e.to_string())?);
                Ok(())
            }
            _ => config.set_option(key, value),
        })?;
    }
    let config = loaded()?;
    match (key, config.options().remove(key)) {
        (FIT, _) => Ok(config.resolve("").fit.to_string()),
        (BATTERY, _) => Ok(on_battery(config.playback.max_fps).to_string()),
        (_, Some(toml::Value::String(text))) => Ok(text),
        (_, Some(value)) => Ok(value.to_string()),
        (_, None) => Err(Error::Setting(format!("unknown setting {key}"))),
    }
}

pub(crate) fn chosen(key: &str) -> String {
    setting(key, None).unwrap_or_default()
}

fn on_battery(max_fps: u32) -> u32 {
    if max_fps == DISPLAY { power::BATTERY_FPS } else { max_fps }
}

fn caps() {
    let config = config();
    power::caps(on_battery(config.playback.max_fps), chosen(CHARGER).parse().unwrap_or(60));
    power::guard(config.playback.flash_guard, config.playback.scene_capture);
}

pub(crate) fn look() -> Look {
    let config = config();
    let shown = config.resolve("");
    Look { fit: shown.fit, background: shown.background, fade: config.transition.duration() }
}
