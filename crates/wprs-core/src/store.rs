use crate::config::Config;
use std::io::Write;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("cannot write {path}: {source}")]
    Write { path: PathBuf, source: std::io::Error },
    #[error("invalid configuration at line {line}, column {column}: {message}")]
    Parse { line: usize, column: usize, message: String },
    #[error("cannot serialize configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("no configuration directory could be determined")]
    NoConfigDir,
}

pub fn stopped_effects() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("wallpaper-rs").join("stopped-effects"))
}

impl Config {
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        Ok(dirs::config_dir().ok_or(ConfigError::NoConfigDir)?.join("wallpaper-rs").join("config.toml"))
    }

    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(source) => return Err(ConfigError::Read { path: path.into(), source }),
        };
        let mut config: Config = toml::from_str(&text).map_err(|e| ConfigError::parse(&text, &e))?;
        for key in config.normalize() {
            tracing::warn!(key, "invalid value replaced by the nearest valid one");
        }
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        write_private(path, toml::to_string_pretty(self)?.as_bytes())
    }

    pub fn backup_broken(path: &Path) -> Result<PathBuf, ConfigError> {
        let data = std::fs::read(path).map_err(|source| ConfigError::Read { path: path.into(), source })?;
        let backup = path.with_added_extension("broken");
        write_private(&backup, &data).map(|()| backup)
    }
}

impl ConfigError {
    fn parse(text: &str, error: &toml::de::Error) -> ConfigError {
        let before = text.get(..error.span().map_or(0, |span| span.start)).unwrap_or_default();
        let line = before.split('\n').count();
        let column = before.rsplit('\n').next().unwrap_or_default().chars().count() + 1;
        ConfigError::Parse { line, column, message: error.message().trim().replace('\n', "; ") }
    }
}

pub(crate) fn write_private(path: &Path, data: &[u8]) -> Result<(), ConfigError> {
    let target = std::fs::canonicalize(path)
        .or_else(|_| std::fs::read_link(path).map(|link| path.with_file_name(link)))
        .unwrap_or_else(|_| path.to_owned());
    let write_err = |source| ConfigError::Write { path: target.clone(), source };
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(write_err)?;
    let mut file = tempfile::Builder::new().prefix(".wprs-").tempfile_in(dir).map_err(write_err)?;
    file.write_all(data).map_err(write_err)?;
    file.persist(&target).map_err(|e| write_err(e.error))?;
    Ok(())
}
