mod db;
mod edits;
mod entries;
mod filter;
mod import;
mod items;
mod packages;
mod playlists;
mod profiles;
mod query;
mod resolve;
mod rotation;
mod rules;
mod scan;
mod settings;
mod tags;
mod timing;

pub use db::SCHEMA_VERSION;
pub use entries::EntryEdit;
pub use filter::query_keys;
pub use import::qhash;
pub use items::{Item, Meta, PackageInfo};
pub use playlists::PlaylistInfo;
pub use profiles::{Preset, Profile};
pub use resolve::Resolved;
pub use rotation::{Rotation, Used};
pub use rules::Rules;
pub use scan::{ScanReport, Source};
pub use timing::{Turn, rotate};

use rusqlite::Connection;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::str::FromStr;

const IMAGES: [&str; 7] = ["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif"];
const VIDEOS: [&str; 7] = ["mp4", "m4v", "webm", "mkv", "mov", "avi", "ogv"];
const DEPTH_MAP: &str = ".depth.png";

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0} {1:?} not found")]
    NotFound(&'static str, String),
    #[error("{0} {1:?} already exists")]
    Exists(&'static str, String),
    #[error("invalid {0}: {1}")]
    Invalid(&'static str, String),
    #[error("invalid query: {0}")]
    Query(String),
    #[error("the database uses schema version {0} but this build only knows up to {1}")]
    Newer(u32, u32),
    #[error("the playlist changed; reload it")]
    Conflict,
    #[error("cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Video,
    Package,
}

impl Kind {
    pub fn detect(path: &Path) -> Option<Kind> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            _ if name.ends_with(DEPTH_MAP) => None,
            e if IMAGES.contains(&e) => Some(Kind::Image),
            e if VIDEOS.contains(&e) => Some(Kind::Video),
            "wpak" => Some(Kind::Package),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Video => "video",
            Kind::Package => "package",
        }
    }
}

impl FromStr for Kind {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        [Kind::Image, Kind::Video, Kind::Package]
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| Error::Invalid("kind", format!("{text:?} is not image, video or package")))
    }
}

impl ToSql for Kind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for Kind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str()?.parse().map_err(|_| FromSqlError::InvalidType)
    }
}

#[derive(Debug)]
pub struct Library {
    db: Connection,
}
