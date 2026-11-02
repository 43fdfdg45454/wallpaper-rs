use crate::auth::Auth;
use crate::db::{HOUR, now};
use crate::error::{Input, Problem, Res};
use crate::{Reply, Router, St};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Map, Value, json};

const PER_HOUR: i64 = 60;
const COUNTS: &str = "SELECT COALESCE(SUM(value = 1), 0), COALESCE(SUM(value = -1), 0) FROM votes \
                      WHERE package_id = ?1";

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/packages/{id}/vote", get(mine).post(vote))
        .route("/api/v1/votes", get(counts))
}

#[derive(Deserialize)]
struct Vote {
    vote: String,
}

#[derive(Deserialize)]
struct Ids {
    #[serde(default)]
    ids: String,
}

fn published(c: &Connection, id: &str) -> Res<()> {
    let sql = "SELECT COUNT(*) FROM versions v JOIN packages p ON p.id = v.package_id \
               WHERE v.package_id = ?1 AND v.state = 'published' AND p.hidden IS NULL";
    match c.query_row(sql, [id], |r| r.get::<_, i64>(0))? {
        0 => Err(Problem::missing("published package")),
        _ => Ok(()),
    }
}

fn tally(c: &Connection, id: &str, user: Option<i64>) -> Res<Value> {
    let (up, down): (i64, i64) = c.query_row(COUNTS, [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut found = json!({"id": id, "up": up, "down": down});
    if let Some(user) = user {
        let sql = "SELECT COALESCE((SELECT value FROM votes WHERE package_id = ?1 AND user_id = ?2), 0)";
        let mine: i64 = c.query_row(sql, params![id, user], |r| r.get(0))?;
        found["mine"] = json!(match mine {
            1 => "up",
            -1 => "down",
            _ => "none",
        });
    }
    Ok(found)
}

async fn mine(State(app): St, auth: Auth, Path(id): Path<String>) -> Reply {
    Ok(Json(app.db.run(move |c| published(c, &id).and_then(|()| tally(c, &id, Some(auth.user)))).await?))
}

async fn vote(State(app): St, auth: Auth, Path(id): Path<String>, Input(v): Input<Vote>) -> Reply {
    let value = match v.vote.as_str() {
        "up" => 1,
        "down" => -1,
        "none" => 0,
        _ => return Err(Problem::bad("vote: up, down or none")),
    };
    let user = auth.user;
    let found = app.db.run(move |c| {
        published(c, &id)?;
        let t = now();
        c.execute("DELETE FROM vote_changes WHERE at < ?1", [t - HOUR])?;
        let sql = "SELECT COUNT(*) FROM vote_changes WHERE user_id = ?1";
        if c.query_row(sql, [user], |r| r.get::<_, i64>(0))? >= PER_HOUR {
            return Err(Problem::busy(600));
        }
        c.execute("INSERT INTO vote_changes (user_id, at) VALUES (?1, ?2)", params![user, t])?;
        match value {
            0 => c.execute("DELETE FROM votes WHERE package_id = ?1 AND user_id = ?2", params![id, user])?,
            _ => c.execute(
                "INSERT INTO votes (package_id, user_id, value, at) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (package_id, user_id) DO UPDATE SET value = ?3, at = ?4",
                params![id, user, value, t],
            )?,
        };
        tally(c, &id, Some(user))
    });
    Ok(Json(found.await?))
}

async fn counts(State(app): St, auth: Option<Auth>, Query(q): Query<Ids>) -> Reply {
    let ids: Vec<String> = q.ids.split(',').filter(|i| !i.is_empty()).map(String::from).collect();
    if ids.len() > 100 || !ids.iter().all(|i| wprs_format::valid_id(i)) {
        return Err(Problem::bad("ids: at most 100 package ids separated by commas"));
    }
    let items = app.db.run(move |c| {
        let mut items = Map::new();
        for id in ids {
            let mut found = tally(c, &id, auth.as_ref().map(|a| a.user))?;
            found.as_object_mut().map(|o| o.remove("id"));
            items.insert(id, found);
        }
        Ok(items)
    });
    Ok(Json(json!({"items": items.await?})))
}

pub fn ballot(id: &str, latest: &Value, signed: bool) -> maud::Markup {
    let (up, down) = (&latest["votes"]["up"], &latest["votes"]["down"]);
    maud::html! {
        p {
            @if latest["featured"] == true { strong { "Featured by this server. " } }
            "▲ " (up) " · ▼ " (down) " votes, counted without any list of who voted."
        }
        @if signed { @for (vote, label) in [("up", "Vote up"), ("down", "Vote down"), ("none", "Withdraw my vote")] {
            form.inline method="post" action={ "/api/v1/packages/" (id) "/vote?back=/packages/" (id) } {
                input type="hidden" name="vote" value=(vote);
                button { (label) }
            }
        } }
    }
}
