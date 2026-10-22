use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Playback {
    pub volume: f64,
    pub muted: bool,
    pub max_fps: u32,
}

impl Default for Playback {
    fn default() -> Self {
        Self { volume: 0.5, muted: true, max_fps: 30 }
    }
}

impl Playback {
    pub const MAX_FPS: u32 = 240;

    pub fn sanitized(&self) -> Playback {
        Playback {
            volume: if self.volume.is_finite() { self.volume.clamp(0.0, 1.0) } else { 0.0 },
            muted: self.muted,
            max_fps: self.max_fps.clamp(1, Self::MAX_FPS),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionKind {
    #[default]
    Fade,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration_ms: u32,
}

impl Default for Transition {
    fn default() -> Self {
        Self { kind: TransitionKind::Fade, duration_ms: 600 }
    }
}

impl Transition {
    pub const MAX_MS: u32 = 10_000;

    pub fn duration(&self) -> Duration {
        match self.kind {
            TransitionKind::Fade => Duration::from_millis(u64::from(self.duration_ms.min(Self::MAX_MS))),
            TransitionKind::None => Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pause {
    pub when_covered: bool,
    pub on_battery: bool,
    pub when_locked: bool,
}

impl Default for Pause {
    fn default() -> Self {
        Self { when_covered: true, on_battery: true, when_locked: true }
    }
}

const OPTION_SECTIONS: [&str; 3] = ["playback", "transition", "pause"];

impl Config {
    pub fn options(&self) -> BTreeMap<String, toml::Value> {
        let doc = toml::Value::try_from(self).unwrap_or(toml::Value::Table(Default::default()));
        OPTION_SECTIONS
            .iter()
            .filter_map(|section| Some((section, doc.get(section)?.as_table()?)))
            .flat_map(|(section, table)| {
                table.iter().map(move |(k, v)| (format!("{section}.{k}"), v.clone()))
            })
            .collect()
    }

    pub fn set_option(&mut self, key: &str, value: &str) -> Result<(), String> {
        let unknown = || format!("unknown setting '{key}', see the list of settings");
        let current = self.options().remove(key).ok_or_else(unknown)?;
        let text = value.trim();
        let parsed = match current {
            toml::Value::Boolean(_) => text.parse().map(toml::Value::Boolean).map_err(|e| e.to_string()),
            toml::Value::Integer(_) => text.parse().map(toml::Value::Integer).map_err(|e| e.to_string()),
            toml::Value::Float(_) => text.parse().map(toml::Value::Float).map_err(|e| e.to_string()),
            _ => Ok(toml::Value::String(text.to_owned())),
        }
        .map_err(|e| format!("invalid value for {key}: {e}"))?;
        let (section, field) = key.split_once('.').ok_or_else(unknown)?;
        let mut doc = toml::Value::try_from(&*self).map_err(|e| e.to_string())?;
        doc[section][field] = parsed;
        let mut updated: Config = doc.try_into().map_err(|e| format!("invalid value for {key}: {e}"))?;
        if updated.normalize().iter().any(|fixed| fixed == key) {
            return Err(format!("{text} is out of range for {key}"));
        }
        *self = updated;
        Ok(())
    }
}
