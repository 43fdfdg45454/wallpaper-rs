use crate::fit::Fit;
use crate::settings::{Pause, Playback, Transition};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("cannot write {path}: {source}")]
    Write { path: PathBuf, source: std::io::Error },
    #[error("invalid configuration in {path}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },
    #[error("cannot serialize configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("no configuration directory could be determined")]
    NoConfigDir,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutputConfig {
    pub image: Option<PathBuf>,
    pub fit: Option<Fit>,
    pub background: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub default: OutputConfig,
    pub outputs: BTreeMap<String, OutputConfig>,
    pub playback: Playback,
    pub transition: Transition,
    pub pause: Pause,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub image: Option<PathBuf>,
    pub fit: Fit,
    pub background: [u8; 3],
}

impl Config {
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        Ok(dirs::config_dir().ok_or(ConfigError::NoConfigDir)?.join("wallpaper-rs").join("config.toml"))
    }

    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|source| ConfigError::Parse { path: path.into(), source })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(source) => Err(ConfigError::Read { path: path.into(), source }),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self)?;
        let write_err = |source| ConfigError::Write { path: path.into(), source };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(write_err)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(write_err)?;
        std::fs::rename(&tmp, path).map_err(write_err)
    }

    pub fn resolve(&self, output: &str) -> Resolved {
        let specific = self.outputs.get(output);
        let pick = |f: fn(&OutputConfig) -> Option<_>| specific.and_then(f).or_else(|| f(&self.default));
        Resolved {
            image: specific.and_then(|o| o.image.clone()).or_else(|| self.default.image.clone()),
            fit: pick(|o| o.fit).unwrap_or_default(),
            background: specific
                .and_then(|o| o.background.as_deref())
                .or(self.default.background.as_deref())
                .and_then(parse_hex_color)
                .unwrap_or([0, 0, 0]),
        }
    }

    pub fn set_image(&mut self, output: &str, image: PathBuf, fit: Option<Fit>) {
        let entry = if output.is_empty() {
            self.outputs.values_mut().for_each(|o| o.image = None);
            &mut self.default
        } else {
            self.outputs.entry(output.to_owned()).or_default()
        };
        entry.image = Some(image);
        entry.fit = fit.or(entry.fit);
    }

    pub fn images(&self) -> impl Iterator<Item = &PathBuf> {
        std::iter::once(&self.default).chain(self.outputs.values()).filter_map(|o| o.image.as_ref())
    }
}

pub fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    (s.len() == 6 && s.is_ascii()).then(|| Some([byte(0)?, byte(2)?, byte(4)?])).flatten()
}
