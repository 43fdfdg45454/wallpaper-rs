use crate::compositor::lock;
use crate::error::{Error, Result, failed};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use wprs_core::{Config, OutputConfig};
use wprs_library::Library;

pub struct Store {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub error: Option<String>,
    writer: Mutex<Option<Library>>,
    reader: Mutex<Option<Library>>,
}

pub fn private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(path)
}

impl Store {
    pub fn open() -> Store {
        let dir = |base: Option<PathBuf>| base.unwrap_or_else(|| PathBuf::from(".")).join("wallpaper-rs");
        let (data, cache) = (dir(dirs::data_dir()), dir(dirs::cache_dir()));
        let path = data.join("library.db");
        let opened = Library::open(&path).and_then(|w| Ok((w, Library::open_read_only(&path)?)));
        let (connections, error) = match opened {
            Ok((writer, reader)) => (Some((writer, reader)), None),
            Err(e) => {
                tracing::error!("cannot open the library: {e}");
                (None, Some(format!("cannot open the library: {e}")))
            }
        };
        let (writer, reader) = connections.unzip();
        Store { data, cache, error, writer: Mutex::new(writer), reader: Mutex::new(reader) }
    }

    pub fn files(&self) -> PathBuf {
        self.data.join("files")
    }

    fn unavailable(&self) -> Error {
        failed(self.error.clone().unwrap_or_else(|| "the library is not available".into()))
    }

    pub fn write<T, E: Into<Error>>(&self, work: impl FnOnce(&mut Library) -> Result<T, E>) -> Result<T> {
        work(lock(&self.writer).as_mut().ok_or_else(|| self.unavailable())?).map_err(Into::into)
    }

    pub fn read<T, E: Into<Error>>(&self, work: impl FnOnce(&Library) -> Result<T, E>) -> Result<T> {
        work(lock(&self.reader).as_ref().ok_or_else(|| self.unavailable())?).map_err(Into::into)
    }

    pub fn connect(&self) -> Result<Library> {
        self.error.as_ref().map_or(Ok(()), |_| Err(self.unavailable()))?;
        Ok(Library::open(self.data.join("library.db"))?)
    }

    pub fn drop_dangling(&self, config: &mut Config) -> bool {
        let Ok(dangling) = self.read(|lib| Ok::<_, Error>(dangling(lib, config))) else { return false };
        for key in &dangling {
            tracing::warn!(key, "dropped a reference to something the library no longer has");
        }
        !dangling.is_empty()
    }
}

fn dangling(lib: &Library, config: &mut Config) -> Vec<String> {
    let gone = |e: wprs_library::Result<()>| matches!(e, Err(wprs_library::Error::NotFound(..)));
    let mut dropped = Vec::new();
    if config.profile.as_deref().is_some_and(|uid| gone(lib.profile(uid).map(drop))) {
        config.profile = None;
        dropped.push("profile".to_owned());
    }
    let sections = std::iter::once(("default".to_owned(), &mut config.default))
        .chain(config.outputs.iter_mut().map(|(name, o)| (format!("outputs.{name}"), o)));
    for (section, OutputConfig { playlist, preset, .. }) in sections {
        if playlist.as_deref().is_some_and(|uid| gone(lib.playlist(uid).map(drop))) {
            *playlist = None;
            dropped.push(format!("{section}.playlist"));
        }
        if preset.as_deref().is_some_and(|uid| gone(lib.preset(uid).map(drop))) {
            *preset = None;
            dropped.push(format!("{section}.preset"));
        }
    }
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_to_missing_rows_are_dropped() {
        let mut lib = Library::open_in_memory().unwrap();
        let preset = lib.create_preset(None, "Dim", &serde_json::json!({"fit": "fit"})).unwrap();
        let mut config = Config { profile: Some("01J0000000000000000000GONE".into()), ..Config::default() };
        config.default.preset = Some(preset);
        config.outputs.entry("DP-1".into()).or_default().playlist = Some("01J0000000000000000000GONE".into());
        assert_eq!(dangling(&lib, &mut config), ["profile", "outputs.DP-1.playlist"]);
        assert!(config.default.preset.is_some() && config.profile.is_none());
        assert!(dangling(&lib, &mut config).is_empty());
    }
}
