use crate::config::Config;
use crate::db::{migrate, now};
use anyhow::{Context, Result, ensure};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, DirBuilder};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use wprs_format::sha256_file;

const FORMAT: &str = "wprs-server-backup/1";
const TREES: [&str; 4] = ["blobs", "thumbs", "keys", "repo"];

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    created_at: i64,
    files: BTreeMap<String, String>,
}

pub fn backup(cfg: &Config, dest: Option<PathBuf>) -> Result<PathBuf> {
    let stamp = chrono::DateTime::from_timestamp_millis(now()).unwrap_or_default().format("%Y%m%d-%H%M%S");
    let dest = dest.unwrap_or_else(|| cfg.data.join("backups").join(stamp.to_string()));
    ensure!(fs::read_dir(&dest).map_or(true, |mut d| d.next().is_none()), "{} is not empty", dest.display());
    DirBuilder::new().recursive(true).mode(0o700).create(&dest)?;
    let db = Connection::open(cfg.data.join("db/wprs.sqlite"))?;
    db.busy_timeout(Duration::from_secs(30))?;
    db.pragma_update(None, "temp_store", "MEMORY")?;
    let snapshot = dest.join("wprs.sqlite");
    db.execute("VACUUM INTO ?1", [snapshot.to_str().context("the backup path is not UTF-8")?])?;
    let mut files = BTreeMap::from([("wprs.sqlite".to_string(), sha256_file(&snapshot)?)]);
    for tree in TREES {
        copy(&cfg.data.join(tree), &dest.join(tree), tree, &mut files)?;
    }
    let manifest = Manifest { format: FORMAT.into(), created_at: now(), files };
    fs::write(dest.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(dest)
}

fn copy(from: &Path, to: &Path, prefix: &str, files: &mut BTreeMap<String, String>) -> Result<()> {
    let Ok(items) = fs::read_dir(from) else { return Ok(()) };
    DirBuilder::new().recursive(true).mode(0o700).create(to)?;
    for item in items {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        let (source, target, key) = (item.path(), to.join(&name), format!("{prefix}/{name}"));
        let kind = item.file_type()?;
        if kind.is_dir() {
            copy(&source, &target, &key, files)?;
        } else if kind.is_file() && !name.starts_with(".tmp-") {
            fs::copy(&source, &target)?;
            files.insert(key, sha256_file(&target)?);
        }
    }
    Ok(())
}

pub fn restore(cfg: &Config, src: &Path, force: bool) -> Result<()> {
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(src.join("manifest.json")).context("no manifest.json in the backup")?,
    )?;
    ensure!(manifest.format == FORMAT, "unknown backup format {}", manifest.format);
    for (path, sha) in &manifest.files {
        let tree = path.split('/').next().unwrap_or_default();
        let known = path == "wprs.sqlite" || (TREES.contains(&tree) && path.len() > tree.len());
        let safe = known && !path.split('/').any(|s| s == ".." || s == "." || s.is_empty());
        ensure!(safe, "unsafe path {path} in the manifest");
        ensure!(sha256_file(src.join(path))? == *sha, "{path} does not match its checksum");
    }
    let check: String =
        Connection::open(src.join("wprs.sqlite"))?.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    ensure!(check == "ok", "the database snapshot is damaged: {check}");
    let db = cfg.data.join("db/wprs.sqlite");
    ensure!(
        force || !db.exists(),
        "{} already holds a database: stop the server and pass --force to replace it",
        cfg.data.display()
    );
    let repo = cfg.data.join("repo");
    let root = |dir: &Path| fs::read(dir.join("root/1.json")).ok();
    let same = root(&repo).is_some_and(|live| root(&src.join("repo")) == Some(live));
    let seen = latest(&repo).filter(|_| same);
    for item in fs::read_dir(&repo).into_iter().flatten() {
        let item = item?;
        let signed = ["root", "entry", "index", "revoked"].iter().any(|d| item.file_name() == *d);
        match item.file_type()?.is_dir() {
            _ if same && signed => {}
            true => fs::remove_dir_all(item.path())?,
            false => fs::remove_file(item.path())?,
        }
    }
    for path in manifest.files.keys().filter(|p| *p != "wprs.sqlite") {
        let target = cfg.data.join(path);
        DirBuilder::new().recursive(true).mode(0o700).create(target.parent().context("bad path")?)?;
        fs::copy(src.join(path), &target)?;
    }
    DirBuilder::new().recursive(true).mode(0o700).create(cfg.data.join("db"))?;
    let staged = cfg.data.join("db/restore.sqlite");
    fs::copy(src.join("wprs.sqlite"), &staged)?;
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", db.display()));
    }
    fs::rename(&staged, &db)?;
    if let Some(n) = seen.filter(|n| latest(&repo).is_none_or(|r| r < *n)) {
        fs::write(repo.join("entry/latest"), format!("{n}\n"))?;
    }
    let mut conn = Connection::open(&db)?;
    migrate(&mut conn)?;
    conn.execute("UPDATE repo_state SET dirty = 1", [])?;
    conn.execute("UPDATE sync_heads SET seq = MAX(seq, ?1), min_seq = MAX(seq, ?1)", [now()])?;
    Ok(())
}

fn latest(repo: &Path) -> Option<u64> {
    fs::read_to_string(repo.join("entry/latest")).ok()?.trim().parse().ok()
}
