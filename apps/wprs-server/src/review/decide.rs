use super::{Decision, REASONS, keep};
use crate::App;
use crate::db::{DAY, audit, now};
use crate::error::{Problem, Res, require};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::fs;
use std::sync::Arc;

pub async fn decide(app: &Arc<App>, actor: Option<i64>, case: i64, d: Decision) -> Res<Value> {
    let choice = d.decision.as_str();
    let restrictive = matches!(choice, "reject" | "remove" | "revoke");
    require(
        restrictive || matches!(choice, "approve" | "restore" | "dismiss"),
        "decision: approve, reject, remove, revoke, restore or dismiss",
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
    if actor.is_some() && actor == owner && !restrictive {
        return Err(Problem::forbidden("moderators cannot approve, restore or dismiss their own uploads"));
    }
    let needs = |from: &str, to: &'static str| match state == from {
        true => Ok(Some(to)),
        false => Err(Problem::conflict("version_state", format!("the version is {state}, not {from}"))),
    };
    let (shared, blob) = (app.clone(), app.cfg.data.join("blobs").join(format!("{sha}.wpak")));
    let held = |ext: &str| app.cfg.data.join("held").join(format!("{sha}.{ext}"));
    let (id, number, path) = (package.clone(), version.clone(), blob.clone());
    let key = package.clone();
    let hidden = "SELECT hidden IS NOT NULL FROM packages WHERE id = ?1";
    let hidden: bool =
        app.db.run(move |c| Ok(c.query_row(hidden, [key], |r| r.get(0)).optional()?)).await?.unwrap_or(false);
    let next = match choice {
        "approve" if !app.repo.ready() => {
            return Err(Problem::conflict(
                "no_repository",
                "create the repository first: wprs-server repo init",
            ));
        }
        "approve" => {
            let next = needs("pending", "published")?;
            tokio::task::spawn_blocking(move || shared.repo.add(&path, now(), hidden)).await??;
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
        "restore" => {
            let next = needs(if state == "revoked" { "revoked" } else { "removed" }, "published")?;
            let (kept, t) = (held("wpak"), now());
            if !kept.is_file() {
                return Err(Problem::conflict(
                    "deleted",
                    "the withdrawn file was deleted after its retention",
                ));
            }
            tokio::task::spawn_blocking(move || shared.repo.restore(&id, &number, &kept, t, hidden))
                .await??;
            next
        }
        _ => None,
    };
    let (statement, strike, decision) =
        (d.statement.trim().to_string(), d.strike && restrictive, d.decision.clone());
    let (subject, closed) =
        (format!("{package}@{version}"), if choice == "dismiss" { "dismissed" } else { "actioned" });
    let (logged, key) = (subject.clone(), sha.clone());
    let shown = app.db.run(move |c| {
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
        let sql = "SELECT COUNT(*) FROM versions WHERE sha256 = ?1 AND state IN ('pending', 'published')";
        let shown: i64 = tx.query_row(sql, [&key], |r| r.get(0))?;
        tx.commit()?;
        Ok(shown > 0)
    });
    let shown = shown.await?;
    let data = &app.cfg.data;
    let (package, thumb) = (
        data.join("repo/packages").join(&package).join(format!("{version}.wpak")),
        data.join("thumbs").join(format!("{sha}.jpg")),
    );
    match next {
        _ if choice == "restore" => {
            let _ = fs::rename(held("jpg"), &thumb);
            let _ = fs::remove_file(held("wpak"));
        }
        Some("published") => drop(fs::remove_file(blob)),
        Some("rejected") => {
            [blob].into_iter().chain((!shown).then_some(thumb)).for_each(|f| drop(fs::remove_file(f)))
        }
        Some(_) => {
            for (from, ext) in [(package, "wpak")].into_iter().chain((!shown).then_some((thumb, "jpg"))) {
                if let Err(e) = keep(&from, &held(ext)) {
                    tracing::warn!("a withdrawn file could not be kept: {e}");
                }
            }
        }
        None => {}
    }
    if next.is_some_and(|s| s != "rejected") {
        app.dirty.notify_one();
    }
    Ok(json!({"case": case, "decision": d.decision, "subject": subject, "state": next.unwrap_or(&state)}))
}
