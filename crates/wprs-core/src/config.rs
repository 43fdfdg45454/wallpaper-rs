//! Daemon configuration stored in `$XDG_CONFIG_HOME/wallpaper-rs/config.toml`.

use crate::fit::Fit;
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

/// Wallpaper settings for one output, or the defaults for all outputs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutputConfig {
    /// Image file to show. `None` shows the background color only.
    pub image: Option<PathBuf>,
    /// Placement of the image.
    pub fit: Option<Fit>,
    /// Background color as `#rrggbb`, used for bars and when no image is set.
    pub background: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Settings used for every output without its own entry.
    pub default: OutputConfig,
    /// Per-output overrides keyed by connector name, e.g. `DP-1`.
    pub outputs: BTreeMap<String, OutputConfig>,
}

/// Fully resolved settings for one output.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub image: Option<PathBuf>,
    pub fit: Fit,
    pub background: [u8; 3],
}

impl Config {
    /// Default location of the configuration file.
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        let dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(dir.join("wallpaper-rs").join("config.toml"))
    }

    /// Loads the file at `path`. A missing file yields the default configuration.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| ConfigError::Parse {
                path: path.to_owned(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(source) => Err(ConfigError::Read {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Writes the configuration atomically (temporary file + rename).
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self)?;
        let write_err = |source| ConfigError::Write {
            path: path.to_owned(),
            source,
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(write_err)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(write_err)?;
        std::fs::rename(&tmp, path).map_err(write_err)
    }

    /// Settings for `output`, falling back to the defaults field by field.
    pub fn resolve(&self, output: &str) -> Resolved {
        let specific = self.outputs.get(output);
        let pick = |f: fn(&OutputConfig) -> Option<_>| specific.and_then(f).or_else(|| f(&self.default));
        let image = specific
            .and_then(|o| o.image.clone())
            .or_else(|| self.default.image.clone());
        let fit = pick(|o| o.fit).unwrap_or_default();
        let background = specific
            .and_then(|o| o.background.as_deref())
            .or(self.default.background.as_deref())
            .and_then(parse_hex_color)
            .unwrap_or([0, 0, 0]);
        Resolved {
            image,
            fit,
            background,
        }
    }

    /// Sets the image for `output`, or for every output when `output` is empty.
    /// Setting the default clears per-output images so the change is visible everywhere.
    pub fn set_image(&mut self, output: &str, image: PathBuf, fit: Option<Fit>) {
        if output.is_empty() {
            self.default.image = Some(image);
            if fit.is_some() {
                self.default.fit = fit;
            }
            for o in self.outputs.values_mut() {
                o.image = None;
            }
        } else {
            let entry = self.outputs.entry(output.to_owned()).or_default();
            entry.image = Some(image);
            if fit.is_some() {
                entry.fit = fit;
            }
        }
    }
}

/// Parses `#rrggbb` or `rrggbb`.
pub fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_default() {
        let c = Config::load(Path::new("/nonexistent/wallpaper-rs/config.toml")).unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn resolve_falls_back_per_field() {
        let c: Config = toml::from_str(
            r##"
            [default]
            image = "/a.png"
            fit = "fit"
            background = "#102030"

            [outputs.DP-1]
            image = "/b.png"
            "##,
        )
        .unwrap();
        let dp = c.resolve("DP-1");
        assert_eq!(dp.image.as_deref(), Some(Path::new("/b.png")));
        assert_eq!(dp.fit, Fit::Fit);
        assert_eq!(dp.background, [0x10, 0x20, 0x30]);
        let other = c.resolve("HDMI-A-1");
        assert_eq!(other.image.as_deref(), Some(Path::new("/a.png")));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(toml::from_str::<Config>("[default]\nimgae = \"/x\"").is_err());
    }

    #[test]
    fn set_image_for_all_clears_overrides() {
        let mut c = Config::default();
        c.set_image("DP-1", "/b.png".into(), None);
        c.set_image("", "/a.png".into(), Some(Fit::Center));
        assert_eq!(c.resolve("DP-1").image.as_deref(), Some(Path::new("/a.png")));
        assert_eq!(c.resolve("DP-1").fit, Fit::Center);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("wprs-core-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let mut c = Config::default();
        c.set_image("DP-2", "/c.png".into(), Some(Fit::Tile));
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), c);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hex_colors() {
        assert_eq!(parse_hex_color("#ff0080"), Some([255, 0, 128]));
        assert_eq!(parse_hex_color("ff0080"), Some([255, 0, 128]));
        assert_eq!(parse_hex_color("#fff"), None);
        assert_eq!(parse_hex_color("#gg0000"), None);
    }
}
