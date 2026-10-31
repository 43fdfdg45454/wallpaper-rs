mod about;
mod admin;
mod archive;
mod authors;
mod broken;
mod browse;
mod build;
mod catalog;
mod check;
mod client;
mod contents;
mod display;
mod download;
mod entry;
mod facts;
pub mod features;
mod fetch;
mod index;
mod install;
#[cfg(feature = "sign")]
mod keys;
mod locale;
mod manifest;
pub mod media;
mod meta;
mod names;
mod needs;
mod online;
mod origin;
#[cfg(feature = "sign")]
mod publish;
mod read;
mod release;
mod repo;
mod said;
mod scenery;
mod settings;
mod sign;
mod source;
mod store;
mod update;
mod upgrade;
mod validate;
pub mod values;
mod video;
mod web;

#[cfg(feature = "sign")]
pub use admin::sign_root;
pub use admin::{RootStatus, init, rotate_root, status};
pub use archive::{extract, peek, thumbnail};
pub use authors::{AUTHORS_FORMAT, Authors, ONLINE, OnlineAuthor, Rotation, Trusted, rotation_text};
pub use browse::{Item, browse};
pub use build::{build, sha256, sha256_file};
pub use catalog::{Added, add, hide, revoke, revoke_key, unrevoke};
pub use client::{Outcome, RepoInfo, Update, add_repo, remove_repo, repos, update_repo, verify};
pub use display::{Accent, Display, Flip, Follow, FrameRate, Span};
pub use download::{preview, previews, release};
pub use entry::{Blob, DETAILS_FORMAT, Details, ENTRY_FORMAT, Entry, Pin};
pub use facts::Facts;
pub use fetch::{Anchor, Fetch, FileFetch};
pub use index::{
    DETAILED_FORMAT, KINDS_FORMAT, KeyRevocation, Listing, REVOKED_FORMAT, Reason, Revocation, Revoked,
    SHARD_FORMAT, Shard, shard_of,
};
pub use install::{Installed, current, install, installed, remove};
#[cfg(feature = "sign")]
pub use keys::{Keypair, Signer, keygen, sign_source};
pub use locale::{LOCALE, locale_file, valid_tag};
pub use manifest::{
    Kind, Manifest, SCHEMA, SCHEMA_V1, Strings, manifest_schema, newer_names, newer_spelling,
};
pub use meta::{Content, Credit, Descriptor, Links, Previews, Rating, Source};
pub use names::valid_id;
pub use needs::{Dependency, Gpu, Input, Network, Permissions, Platform, Requires, origin};
#[cfg(feature = "sign")]
pub use online::sign_rotation;
pub use online::{anchors, drop_author_key, register_author, rotate_author};
pub use origin::{Ancestor, Origin, OriginKind, PROVIDER_LICENSES, Provider};
#[cfg(feature = "sign")]
pub use publish::{Published, publish};
pub use read::{Package, open, read};
pub use release::{Needs, Preview, Rated, Release, Wanted};
pub use repo::{Author, Key, Policy, ROOT_FORMAT, Role, Roles, Root, Time, comment};
pub use settings::{Settings, settings};
pub use sign::{fingerprint, manifest_comment, public_key, threshold, verify_blocks, verify_package};
pub use source::{Folder, fill, inspect, locale_names, with_strings};
pub use upgrade::{Renames, rename_json, upgrade, upgrade_json};
pub use validate::{FileEntry, Limits, MANIFEST, SIGNATURE, check_path, validate};
pub use video::{Playback, VIDEO_PROPERTIES};
pub use web::{PresetFile, Web};
pub use wprs_core::markup;
pub use wprs_scene::{Choice, Condition, Control, Preset, Property, Scene, Slider, Text};

use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}", self.message())]
    Io(#[from] std::io::Error),
    #[error("{}", self.message())]
    Zip(#[from] zip::result::ZipError),
    #[error("{}", self.message())]
    Json(#[from] serde_json::Error),
    #[error("{}", self.message())]
    Invalid { field: String, reason: String },
    #[error("{}", self.message())]
    UnsafePath(String),
    #[error("{}", self.message())]
    Duplicate(String),
    #[error("{}", self.message())]
    Symlink(String),
    #[error("{}", self.message())]
    Missing(String),
    #[error("{}", self.message())]
    Extension { path: String, kind: Kind },
    #[error("{}", self.message())]
    Limit { what: &'static str, actual: u64, limit: u64 },
    #[error("{}", self.message())]
    Unsupported(String, &'static str),
    #[error("{}", self.message())]
    SizeMismatch(String),
    #[error("{}", self.message())]
    Blocked(PathBuf),
    #[error("{}", self.message())]
    Signature(String),
    #[error("{}", self.message())]
    Key(String),
    #[error("{}", self.message())]
    Expired(String),
    #[error("{}", self.message())]
    Stale(String),
    #[error("{}", self.message())]
    Rollback(String),
    #[error("{}", self.message())]
    Equivocation(u64),
    #[error("{}", self.message())]
    Mismatch(String),
    #[error("{}", self.message())]
    Revoked(String),
    #[error("{}", self.message())]
    NotFound(String),
    #[error("{}", self.message())]
    Exists(String),
    #[error("{}", self.message())]
    Newer(String),
    #[error("{}", self.message())]
    Elsewhere(String),
}

pub(crate) fn ensure(ok: bool, error: impl FnOnce() -> Error) -> Result<()> {
    if ok { Ok(()) } else { Err(error()) }
}

pub(crate) fn require(ok: bool, field: &str, reason: &str) -> Result<()> {
    ensure(ok, || Error::Invalid { field: field.into(), reason: reason.into() })
}
