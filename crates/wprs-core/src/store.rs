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
    #[error("invalid configuration in {path}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },
    #[error("cannot serialize configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("no configuration directory could be determined")]
    NoConfigDir,
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
        let mut config: Config =
            toml::from_str(&text).map_err(|source| ConfigError::Parse { path: path.into(), source })?;
        for key in config.normalize() {
            tracing::warn!(key, path = %path.display(), "invalid value replaced by the nearest valid one");
        }
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        write_private(path, toml::to_string_pretty(self)?.as_bytes())
    }

    pub fn backup_broken(path: &Path) -> Result<PathBuf, ConfigError> {
        let data = std::fs::read(path).map_err(|source| ConfigError::Read { path: path.into(), source })?;
        let mut backup = path.as_os_str().to_owned();
        backup.push(".broken");
        write_private(backup.as_ref(), &data).map(|()| backup.into())
    }
}

fn write_private(path: &Path, data: &[u8]) -> Result<(), ConfigError> {
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
