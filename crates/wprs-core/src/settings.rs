use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Playback {
    pub volume: f64,
    pub muted: bool,
    #[serde(with = "fps")]
    pub max_fps: u32,
    pub software_parallax: bool,
}

impl Default for Playback {
    fn default() -> Self {
        Self { volume: 0.5, muted: true, max_fps: fps::DISPLAY, software_parallax: false }
    }
}

pub mod fps {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub const DISPLAY: u32 = 0;
    pub const WORD: &str = "display";

    pub fn serialize<S: Serializer>(fps: &u32, s: S) -> Result<S::Ok, S::Error> {
        match *fps {
            DISPLAY => s.serialize_str(WORD),
            fps => s.serialize_u32(fps),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Fps {
            Number(u32),
            Text(String),
        }
        match Fps::deserialize(d)? {
            Fps::Number(fps) => Ok(fps),
            Fps::Text(text) if text == WORD => Ok(DISPLAY),
            Fps::Text(text) => {
                text.parse().map_err(|_| D::Error::custom(format!("expected 1 to 240 or {WORD}")))
            }
        }
    }
}

impl Playback {
    pub const MAX_FPS: u32 = 240;

    pub fn sanitized(&self) -> Playback {
        Playback {
            volume: if self.volume.is_finite() { self.volume.clamp(0.0, 1.0) } else { 0.0 },
            muted: self.muted,
            max_fps: self.max_fps.min(Self::MAX_FPS),
            software_parallax: self.software_parallax,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Library {
    pub watch: bool,
    pub thumb_cache_mb: u32,
}

impl Default for Library {
    fn default() -> Self {
        Self { watch: true, thumb_cache_mb: 512 }
    }
}

impl Library {
    pub const MIN_MB: u32 = 64;
    pub const MAX_MB: u32 = 8192;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Repos {
    pub auto_refresh: bool,
}

impl Default for Repos {
    fn default() -> Self {
        Self { auto_refresh: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Models {
    pub allow_download: bool,
}

impl Default for Models {
    fn default() -> Self {
        Self { allow_download: true }
    }
}

const OPTION_SECTIONS: [&str; 6] = ["playback", "transition", "pause", "library", "repos", "models"];

impl Config {
    pub fn options(&self) -> BTreeMap<String, toml::Value> {
        let doc = toml::Value::try_from(self).unwrap_or(toml::Value::Table(Default::default()));
        OPTION_SECTIONS
            .iter()
            .filter_map(|section| Some((section, doc.get(section)?.as_table()?)))
            .flat_map(|(section, table)| {
                table.iter().map(move |(k, v)| (format!("{section}.{k}"), v.clone()))
            })
            .chain([("layout.bezel_mm".into(), toml::Value::Float(self.layout.bezel_mm))])
            .collect()
    }

    pub fn set_option(&mut self, key: &str, value: &str) -> Result<(), String> {
        let unknown = || format!("unknown setting '{key}', see the list of settings");
        let current = self.options().remove(key).ok_or_else(unknown)?;
        let text = value.trim();
        let parsed = match current {
            toml::Value::Boolean(_) => text.parse().map(toml::Value::Boolean).map_err(|e| e.to_string()),
            toml::Value::Integer(_) => match text.parse() {
                Ok(number) => Ok(toml::Value::Integer(number)),
                Err(_) if text == fps::WORD => Ok(text.into()),
                Err(e) => Err(e.to_string()),
            },
            toml::Value::Float(_) => text.parse().map(toml::Value::Float).map_err(|e| e.to_string()),
            _ => Ok(toml::Value::String(text.to_owned())),
        }
        .map_err(|e| format!("invalid value for {key}: {e}"))?;
        let (section, field) = key.split_once('.').ok_or_else(unknown)?;
        let mut doc = toml::Value::try_from(&*self).map_err(|e| e.to_string())?;
        let table = doc.as_table_mut().map(|d| d.entry(section).or_insert_with(|| toml::Table::new().into()));
        table.and_then(toml::Value::as_table_mut).ok_or_else(unknown)?.insert(field.into(), parsed);
        let mut updated: Config = doc.try_into().map_err(|e| format!("invalid value for {key}: {e}"))?;
        if updated.normalize().iter().any(|fixed| fixed == key) {
            return Err(format!("{text} is out of range for {key}"));
        }
        *self = updated;
        Ok(())
    }
}
