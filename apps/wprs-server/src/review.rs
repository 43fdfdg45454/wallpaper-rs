use crate::App;
use crate::db::{now, one, rows};
use crate::error::{Problem, Res, require};
use crate::validate::Report;
use rusqlite::params;
use semver::Version;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use wprs_format::Reason;

mod decide;

pub use decide::decide;

const REASONS: [(&str, Reason); 5] = [
    ("malware", Reason::Malware),
    ("license", Reason::License),
    ("takedown", Reason::Takedown),
    ("broken", Reason::Broken),
    ("other", Reason::Other),
];

#[derive(Deserialize, Default)]
pub struct Decision {
    pub decision: String,
    #[serde(default)]
    pub statement: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub strike: bool,
}

fn keep(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)?;
    fs::File::options().write(true).open(to)?.set_modified(SystemTime::now())
}

pub async fn record(app: &App, user: i64, upload: &str, sha: &str, size: i64, report: Report) -> Res<i64> {
    let allowed = &app.cfg.licenses;
    let mut ids =
        report.license.split([' ', '(', ')']).filter(|t| !t.is_empty() && !["AND", "OR", "WITH"].contains(t));
    require(
        ids.all(|l| allowed.contains(&l.to_string())),
        format!("license: this server accepts {}", allowed.join(", ")),
    )?;
    let version = Version::parse(&report.version).map_err(|e| Problem::bad(e.to_string()))?;
    let (tmp, blob) = (app.cfg.data.join("tmp"), app.cfg.data.join("blobs").join(format!("{sha}.wpak")));
    let existed = blob.exists();
    fs::rename(tmp.join(upload), &blob)?;
    fs::rename(tmp.join(format!("{upload}.jpg")), app.cfg.data.join("thumbs").join(format!("{sha}.jpg")))?;
    let (upload, sha, tags, authors) = (
        upload.to_string(),
        sha.to_string(),
        serde_json::to_string(&report.tags)?,
        serde_json::to_string(&report.authors)?,
    );
    let stored = app.db.run(move |c| {
        let (tx, r) = (c.transaction()?, report);
        let owner: Option<Option<i64>> = one(&tx, "SELECT owner_id FROM packages WHERE id = ?1", [&r.id])?;
        if owner.is_some_and(|o| o != Some(user)) {
            return Err(Problem::forbidden("this package id belongs to another account"));
        }
        let sql = "INSERT OR IGNORE INTO packages (id, owner_id, created_at) VALUES (?1, ?2, ?3)";
        tx.execute(sql, params![r.id, user, now()])?;
        let known =
            rows(&tx, "SELECT version FROM versions WHERE package_id = ?1 AND state != 'rejected'", [&r.id])?;
        let newest = known.iter().filter_map(|v| Version::parse(v["version"].as_str()?).ok()).max();
        if let Some(newest) = newest.filter(|n| *n >= version) {
            return Err(Problem::conflict("version", format!("upload a version newer than {newest}")));
        }
        let sql = "INSERT INTO versions (package_id, version, sha256, size, kind, title, description, \
                   license, tags, authors, state, uploaded_by, created_at, content, photosensitive, facets) \
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11, ?12, ?13, ?14, ?15)";
        let (id, number, title, text, license, kind) =
            (r.id, r.version, r.title, r.description, r.license, r.kind.to_string());
        let (content, flashing, facets) = (r.content, r.photosensitive, serde_json::to_string(&r.facets)?);
        let values = params![
            id,
            number,
            sha,
            size,
            kind,
            title,
            text,
            license,
            tags,
            authors,
            user,
            now(),
            content,
            flashing,
            facets
        ];
        tx.execute(sql, values)?;
        let version = tx.last_insert_rowid();
        let sql = "UPDATE uploads SET state = 'done', errors = NULL, version_id = ?2 WHERE id = ?1";
        tx.execute(sql, params![upload, version])?;
        let sql = "INSERT INTO cases (version_id, reason, created_at) VALUES (?1, 'upload', ?2)";
        tx.execute(sql, params![version, now()])?;
        let case = tx.last_insert_rowid();
        tx.commit()?;
        Ok(case)
    });
    let stored = stored.await;
    if stored.is_err() && !existed {
        let _ = fs::remove_file(&blob);
    }
    stored
}

pub async fn approve(app: &Arc<App>, case: i64) -> Res<Value> {
    let statement = "Published automatically for this account.".into();
    decide(app, None, case, Decision { decision: "approve".into(), statement, ..Decision::default() }).await
}
