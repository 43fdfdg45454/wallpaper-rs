use crate::{Error, Library, Result};
use rusqlite::types::Type;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Params, Row};
use serde::de::DeserializeOwned;
use std::fs::{self, DirBuilder, OpenOptions, Permissions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

const MIGRATIONS: [&str; 4] = [
    include_str!("schema.sql"),
    "ALTER TABLE rotation ADD COLUMN current TEXT;",
    "ALTER TABLE items ADD COLUMN content TEXT; ALTER TABLE items ADD COLUMN photosensitive INTEGER; \
     ALTER TABLE items ADD COLUMN facets TEXT; ALTER TABLE items ADD COLUMN pkg_read TEXT; \
     ALTER TABLE items ADD COLUMN titles TEXT; \
     CREATE TABLE confirmations (pkg_id TEXT NOT NULL, what TEXT NOT NULL, PRIMARY KEY (pkg_id, what)) WITHOUT ROWID;",
    "DROP INDEX presets_name; CREATE UNIQUE INDEX presets_name ON presets(coalesce(item_id, 0), \
     coalesce(json_extract(value, '$.package'), ''), name);",
];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

impl Library {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty() && !p.exists()) {
            DirBuilder::new().recursive(true).mode(0o700).create(parent)?;
        }
        OpenOptions::new().create(true).append(true).mode(0o600).open(path)?;
        let library = Self::setup(Connection::open(path)?)?;
        for suffix in ["", "-wal", "-shm"] {
            let mut file = path.as_os_str().to_owned();
            file.push(suffix);
            if Path::new(&file).exists() {
                fs::set_permissions(&file, Permissions::from_mode(0o600))?;
            }
        }
        Ok(library)
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let db = Connection::open_with_flags(path, flags)?;
        match version(&db)? {
            SCHEMA_VERSION => Ok(Self { db }),
            newer if newer > SCHEMA_VERSION => Err(Error::Newer(newer, SCHEMA_VERSION)),
            old => Err(Error::Invalid("database", format!("schema version {old} is not migrated yet"))),
        }
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(mut db: Connection) -> Result<Self> {
        db.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
        )?;
        migrate(&mut db)?;
        Ok(Self { db })
    }
}

fn version(db: &Connection) -> Result<u32> {
    Ok(db.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

fn migrate(db: &mut Connection) -> Result<()> {
    let version = version(db)?;
    ensure(version <= SCHEMA_VERSION, || Error::Newer(version, SCHEMA_VERSION))?;
    let tx = db.transaction()?;
    for (sql, next) in MIGRATIONS.iter().zip(1..).skip(version as usize) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", next)?;
    }
    Ok(tx.commit()?)
}

pub(crate) fn ensure(ok: bool, error: impl FnOnce() -> Error) -> Result<()> {
    if ok { Ok(()) } else { Err(error()) }
}

pub(crate) fn uid() -> String {
    ulid::Ulid::generate().to_string()
}

pub(crate) fn printable(text: &str, max: usize) -> bool {
    !text.is_empty() && text.chars().count() <= max && !text.chars().any(char::is_control)
}

pub(crate) fn label<'a>(what: &'static str, text: &'a str, max: usize) -> Result<&'a str> {
    let text = text.trim();
    let reason = || format!("{text:?} must have 1 to {max} printable characters");
    ensure(printable(text, max), || Error::Invalid(what, reason()))?;
    Ok(text)
}

pub(crate) fn unique<T>(result: rusqlite::Result<T>, what: &'static str, name: &str) -> Result<T> {
    match result {
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            Err(Error::Exists(what, name.into()))
        }
        other => Ok(other?),
    }
}

pub(crate) fn row_id(db: &Connection, what: &'static str, key: &str) -> Result<i64> {
    let sql = match what {
        "playlist" | "profile" => {
            format!("SELECT id FROM {what}s WHERE uid = ?1 OR name = ?1 ORDER BY uid = ?1 DESC LIMIT 1")
        }
        _ => format!("SELECT id FROM {what}s WHERE uid = ?1"),
    };
    db.query_row(&sql, [key], |row| row.get(0)).optional()?.ok_or_else(|| Error::NotFound(what, key.into()))
}

pub(crate) fn item_ids(db: &Connection, uids: &[&str]) -> Result<Vec<i64>> {
    uids.iter().map(|uid| row_id(db, "item", uid)).collect()
}

pub(crate) fn remove(db: &Connection, what: &'static str, key: &str) -> Result<()> {
    let id = row_id(db, what, key)?;
    db.execute(&format!("DELETE FROM {what}s WHERE id = ?1"), [id])?;
    Ok(())
}

pub(crate) fn rows<T>(
    db: &Connection,
    sql: &str,
    params: impl Params,
    map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut statement = db.prepare(sql)?;
    let rows = statement.query_map(params, map)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub(crate) fn json<T: DeserializeOwned>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    let text: String = row.get(index)?;
    serde_json::from_str(&text)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, e.into()))
}
