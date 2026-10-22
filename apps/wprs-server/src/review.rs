use crate::App;
use crate::db::{DAY, audit, now, one, rows};
use crate::error::{Problem, Res, require};
use crate::validate::Report;
use rusqlite::{OptionalExtension, params};
use semver::Version;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;
use std::sync::Arc;
use wprs_format::Reason;

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
                   license, tags, authors, state, uploaded_by, created_at) \
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11, ?12)";
        let (id, number, title, text, license, kind) =
            (r.id, r.version, r.title, r.description, r.license, r.kind.to_string());
        tx.execute(
            sql,
            params![id, number, sha, size, kind, title, text, license, tags, authors, user, now()],
        )?;
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

pub async fn decide(app: &Arc<App>, actor: Option<i64>, case: i64, d: Decision) -> Res<Value> {
    let choice = d.decision.as_str();
    let restrictive = matches!(choice, "reject" | "remove" | "revoke");
    require(
        restrictive || matches!(choice, "approve" | "dismiss"),
        "decision: approve, reject, remove, revoke or dismiss",
    )?;
    require(
        !restrictive || d.statement.trim().chars().count() >= 10,
        "statement: at least 10 characters for the author",
    )?;
    let wanted = d.reason.as_deref().unwrap_or(if choice == "remove" { "takedown" } else { "other" });
    let reason = REASONS.iter().find(|(name, _)| *name == wanted).map(|(_, r)| *r);
    let reason = reason.ok_or_else(|| Problem::bad("reason: malware, license, takedown, broken or other"))?;
    let _turn = app.deciding.lock().await;
    let sql = "SELECT c.state = 'open', v.id, v.package_id, v.version, v.state, v.sha256, v.uploaded_by \
               FROM cases c JOIN versions v ON v.id = c.version_id WHERE c.id = ?1";
    let read =
        |r: &rusqlite::Row| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?));
    let row = app.db.run(move |c| Ok(c.query_row(sql, [case], read).optional()?)).await?;
    let (open, version_id, package, version, state, sha, owner): (
        bool,
        i64,
        String,
        String,
        String,
        String,
        Option<i64>,
    ) = row.ok_or_else(|| Problem::missing("case"))?;
    if !open {
        return Err(Problem::conflict("case_closed", "this case was already decided"));
    }
    let needs = |from: &str, to: &'static str| match state == from {
        true => Ok(Some(to)),
        false => Err(Problem::conflict("version_state", format!("the version is {state}, not {from}"))),
    };
    let (shared, blob) = (app.clone(), app.cfg.data.join("blobs").join(format!("{sha}.wpak")));
    let (id, number, path) = (package.clone(), version.clone(), blob.clone());
    let next = match choice {
        "approve" if !app.repo.ready() => {
            return Err(Problem::conflict(
                "no_repository",
                "create the repository first: wprs-server repo init",
            ));
        }
        "approve" => {
            let next = needs("pending", "published")?;
            tokio::task::spawn_blocking(move || shared.repo.add(&path, now())).await??;
            next
        }
        "reject" => needs("pending", "rejected")?,
        "remove" | "revoke" => {
            let (to, verb) =
                if choice == "remove" { ("removed", "Withdrawn") } else { ("revoked", "Revoked") };
            let next = needs("published", to)?;
            let message = format!("{verb} by the moderators of this server.");
            tokio::task::spawn_blocking(move || shared.repo.revoke(&id, &number, reason, &message)).await??;
            next
        }
        _ => None,
    };
    if matches!(next, Some("published" | "rejected")) {
        let _ = fs::remove_file(&blob);
    }
    let (statement, strike, decision) =
        (d.statement.trim().to_string(), d.strike && restrictive, d.decision.clone());
    let (subject, closed) =
        (format!("{package}@{version}"), if choice == "dismiss" { "dismissed" } else { "actioned" });
    let logged = subject.clone();
    app.db
        .run(move |c| {
            let (tx, t) = (c.transaction()?, now());
            if let Some(to) = next {
                let sql = "UPDATE versions \
                           SET state = ?2, published_at = IIF(?2 = 'published', ?3, published_at) \
                           WHERE id = ?1";
                tx.execute(sql, params![version_id, to, t])?;
                tx.execute("UPDATE repo_state SET dirty = 1 WHERE ?1 != 'rejected'", [to])?;
            }
            let sql = "UPDATE cases \
                       SET state = ?2, decision = ?3, statement = ?4, decided_by = ?5, decided_at = ?6 \
                       WHERE id = ?1 OR (version_id = ?7 AND state = 'open' AND ?2 = 'actioned')";
            tx.execute(sql, params![case, closed, decision, statement, actor, t, version_id])?;
            if let (true, Some(owner)) = (strike, owner) {
                let sql = "INSERT INTO strikes (user_id, case_id, expires_at) VALUES (?1, ?2, ?3)";
                tx.execute(sql, params![owner, case, t + 365 * DAY])?;
            }
            audit(&tx, actor, &decision, &logged, &statement)?;
            Ok(tx.commit()?)
        })
        .await?;
    if next.is_some_and(|s| s != "rejected") {
        app.dirty.notify_one();
    }
    Ok(json!({"case": case, "decision": d.decision, "subject": subject, "state": next.unwrap_or(&state)}))
}
