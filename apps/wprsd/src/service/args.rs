use super::Service;
use crate::daemon::{Daemon, Props};
use crate::error::{Result, failed};
use std::path::PathBuf;
use std::sync::Arc;
use wprs_core::dbus::uri_scheme;
use zbus::fdo::{self, Error::InvalidArgs};

pub(super) fn local_path(path: &str) -> fdo::Result<PathBuf> {
    match uri_scheme(path) {
        None => Ok(path.into()),
        Some(scheme) if scheme.eq_ignore_ascii_case("file") => wprs_video::path_from_uri(path)
            .ok_or_else(|| InvalidArgs(format!("{path} is not a local file URI"))),
        Some(_) => Err(InvalidArgs(format!("{path} is not a local file"))),
    }
}

pub(super) fn absolute(path: String) -> fdo::Result<PathBuf> {
    let path = PathBuf::from(path);
    match path.is_absolute() {
        true => Ok(path),
        false => Err(InvalidArgs(format!("{} is not an absolute path", path.display()))),
    }
}

impl Service {
    pub(super) fn d(&self) -> Result<Arc<Daemon>> {
        self.0.upgrade().ok_or_else(|| failed("the daemon is stopping"))
    }

    pub(super) fn props(&self) -> Props {
        self.0.upgrade().map_or(Props { paused: false, volume: 0.0, muted: true }, |d| *d.props.borrow())
    }
}
