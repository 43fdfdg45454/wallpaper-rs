use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Playback {
    pub volume: f64,
    pub muted: bool,
    #[serde(with = "fps")]
    pub max_fps: u32,
    pub software_parallax: bool,
    pub flash_guard: bool,
    pub scene_sound: bool,
    pub scene_capture: bool,
}

impl Default for Playback {
    fn default() -> Self {
        let (volume, muted, max_fps) = (0.5, true, fps::DISPLAY);
        let (software_parallax, flash_guard, scene_sound, scene_capture) = (false, false, true, true);
        Self { volume, muted, max_fps, software_parallax, flash_guard, scene_sound, scene_capture }
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
            max_fps: self.max_fps.min(Self::MAX_FPS),
            ..self.clone()
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
    pub hide_mature: bool,
}

impl Default for Library {
    fn default() -> Self {
        Self { watch: true, thumb_cache_mb: 512, hide_mature: false }
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
