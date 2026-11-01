use crate::compositor::Settings;
use wprs_core::{Config, Playback, dbus};
use zbus::fdo;

pub(crate) fn settings(config: &Config) -> Settings {
    let playback = config.playback.sanitized();
    let transition = config.transition.duration();
    let Playback { max_fps, volume, muted, software_parallax, flash_guard, scene_sound, .. } = playback;
    let duck = crate::duck::gain();
    Settings { transition, max_fps, volume, muted, software_parallax, flash_guard, scene_sound, duck }
}

pub(crate) fn output_name(output: &str) -> fdo::Result<String> {
    let output = output.trim();
    if output.len() > 128 || output.chars().any(char::is_control) {
        return Err(fdo::Error::InvalidArgs("invalid output name".into()));
    }
    Ok(if output.is_empty() { dbus::DEFAULT_OUTPUT.to_owned() } else { output.to_owned() })
}
