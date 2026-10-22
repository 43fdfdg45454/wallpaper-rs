mod admin;
mod browse;
mod build;
mod catalog;
mod client;
mod entry;
mod fetch;
mod index;
mod install;
#[cfg(feature = "sign")]
mod keys;
mod manifest;
mod property;
#[cfg(feature = "sign")]
mod publish;
mod read;
mod repo;
mod settings;
mod sign;
mod store;
mod update;
mod validate;

#[cfg(feature = "sign")]
pub use admin::sign_root;
pub use admin::{RootStatus, init, rotate_root, status};
pub use browse::{Item, browse};
pub use build::{build, sha256, sha256_file};
pub use catalog::{Added, add, revoke, revoke_key};
pub use client::{Outcome, RepoInfo, Update, add_repo, remove_repo, repos, update_repo, verify};
pub use entry::{Blob, ENTRY_FORMAT, Entry, Pin};
pub use fetch::{Anchor, Fetch, FileFetch};
pub use index::{
    KeyRevocation, Listing, Preview, REVOKED_FORMAT, Reason, Release, Revocation, Revoked, SHARD_FORMAT,
    Shard, shard_of,
};
pub use install::{Installed, install, installed, remove};
#[cfg(feature = "sign")]
pub use keys::{Keypair, Signer, keygen, sign_source};
pub use manifest::{Kind, Manifest, SCHEMA};
pub use property::Property;
#[cfg(feature = "sign")]
pub use publish::{Published, publish};
pub use read::{Package, extract, open, read};
pub use repo::{Author, Key, Policy, ROOT_FORMAT, Role, Roles, Root, Time, comment};
pub use settings::{Settings, settings};
pub use sign::{fingerprint, manifest_comment, public_key, threshold, verify_blocks, verify_package};
pub use validate::{FileEntry, Limits, MANIFEST, SIGNATURE, check_path, validate};

use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid zip archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("malformed JSON: {0}")]
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
    #[error("signature rejected: {0}")]
    Signature(String),
    #[error("invalid key: {0}")]
    Key(String),
    #[error("{0} has expired")]
    Expired(String),
    #[error("repository {0} is stale: its signed index has expired")]
    Stale(String),
    #[error("rollback rejected: {0}")]
    Rollback(String),
    #[error("entry {0} was published twice with different content")]
    Equivocation(u64),
    #[error("{0} does not match its signed hash or size")]
    Mismatch(String),
    #[error("{0} is revoked")]
    Revoked(String),
    #[error("{0} not found")]
    NotFound(String),
}

pub(crate) fn ensure(ok: bool, error: impl FnOnce() -> Error) -> Result<()> {
    if ok { Ok(()) } else { Err(error()) }
}

pub(crate) fn require(ok: bool, field: &str, reason: &str) -> Result<()> {
    ensure(ok, || Error::Invalid { field: field.into(), reason: reason.into() })
}
