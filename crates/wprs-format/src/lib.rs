mod build;
mod manifest;
mod property;
mod read;
mod validate;

pub use build::{build, sha256, sha256_file};
pub use manifest::{Kind, Manifest, SCHEMA};
pub use property::Property;
pub use read::{Package, extract, open, read};
pub use validate::{FileEntry, Limits, MANIFEST, check_path, validate};

use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid zip archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("malformed manifest: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid {field}: {reason}")]
    Invalid { field: String, reason: String },
    #[error("unsafe path {0:?}")]
    UnsafePath(String),
    #[error("duplicate entry {0:?}")]
    Duplicate(String),
    #[error("{0:?} is a symbolic link or special file")]
    Symlink(String),
    #[error("missing file {0:?}")]
    Missing(String),
    #[error("{path:?} has an extension not allowed in {kind} packages")]
    Extension { path: String, kind: Kind },
    #[error("{what} {actual} exceeds the limit of {limit}")]
    Limit { what: &'static str, actual: u64, limit: u64 },
    #[error("entry {0:?} uses an unsupported feature: {1}")]
    Unsupported(String, &'static str),
    #[error("entry {0:?} does not match its declared size")]
    SizeMismatch(String),
    #[error("destination {0:?} is in the way")]
    Blocked(PathBuf),
}

pub(crate) fn ensure(ok: bool, error: impl FnOnce() -> Error) -> Result<()> {
    if ok { Ok(()) } else { Err(error()) }
}

pub(crate) fn require(ok: bool, field: &str, reason: &str) -> Result<()> {
    ensure(ok, || Error::Invalid { field: field.into(), reason: reason.into() })
}
