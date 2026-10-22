use crate::App;
use crate::db::{DAY, HOUR, now};
use crate::error::{Problem, Res};
use rusqlite::params;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use wprs_format::Published;

pub fn spawn(app: Arc<App>) {
    let delay = Duration::from_secs(app.cfg.publish_delay);
    tokio::spawn(async move {
        loop {
            if let Err(problem) = purge(&app).await {
                tracing::warn!(detail = %problem.detail, "cleanup failed");
            }
            if let Err(problem) = republish(&app, false).await {
                tracing::warn!(detail = %problem.detail, "signing the repository failed");
            }
            tokio::select! {
                () = app.dirty.notified() => tokio::time::sleep(delay).await,
                () = tokio::time::sleep(Duration::from_secs(60)) => {}
            }
        }
    });
}

pub async fn republish(app: &Arc<App>, force: bool) -> Res<Option<Published>> {
    if !app.repo.ready() {
        return Ok(None);
    }
    let sql = "SELECT dirty, signed_at FROM repo_state";
    let (dirty, signed) = app
        .db
        .run(move |c| Ok(c.query_row(sql, [], |r| Ok((r.get::<_, bool>(0)?, r.get::<_, i64>(1)?)))?))
        .await?;
    let t = now();
    if !force && !dirty && t - signed < app.cfg.resign_hours as i64 * HOUR {
        return Ok(None);
    }
    app.db.run(|c| Ok(c.execute("UPDATE repo_state SET dirty = 0", [])?)).await?;
    let shared = app.clone();
    let published =
        tokio::task::spawn_blocking(move || shared.repo.publish(shared.cfg.entry_days, t)).await?;
    let sql = "UPDATE repo_state SET dirty = MAX(dirty, ?1), signed_at = IIF(?1, signed_at, ?2)";
    let failed = published.is_err();
    app.db.run(move |c| Ok(c.execute(sql, params![failed, t])?)).await?;
    let published = published.map_err(|e| Problem::conflict("publish", format!("{e:#}")))?;
    tracing::info!(entry = published.version, packages = published.packages, "signed the repository index");
    Ok(Some(published))
}

async fn purge(app: &Arc<App>) -> Res<()> {
    app.clients.purge();
    let t = now();
    let cleaned = app.db.run(move |c| {
        c.execute(
            "DELETE FROM sessions WHERE seen_at < ?1 OR created_at < ?2",
            params![t - 7 * DAY, t - 30 * DAY],
        )?;
        for table in ["device_codes", "setup_tokens", "mailbox", "invites", "strikes"] {
            c.execute(&format!("DELETE FROM {table} WHERE expires_at < ?1"), [t])?;
        }
        let old = t - 180 * DAY;
        let floor = "UPDATE sync_heads SET min_seq = MAX(min_seq, (SELECT COALESCE(MAX(seq), 0) \
                     FROM sync_records r WHERE r.user_id = sync_heads.user_id AND r.deleted = 1 \
                     AND r.updated_at < ?1))";
        c.execute(floor, [old])?;
        c.execute("DELETE FROM sync_records WHERE deleted = 1 AND updated_at < ?1", [old])?;
        Ok(c.execute("DELETE FROM uploads WHERE state != 'done' AND expires_at < ?1", [t])?)
    });
    cleaned.await?;
    let old = |item: &fs::DirEntry| {
        item.metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|m| m.elapsed().unwrap_or_default().as_secs() > 86400)
    };
    for item in fs::read_dir(app.cfg.data.join("tmp"))?.flatten().filter(old) {
        let _ = fs::remove_file(item.path());
    }
    Ok(())
}
