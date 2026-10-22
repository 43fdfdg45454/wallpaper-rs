use crate::auth::Auth;
use crate::db::{DAY, changed, now, one};
use crate::error::{Input, Problem, Res, require};
use crate::{App, Reply, Router, St};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use futures_util::Stream;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::sync::broadcast;

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/mailbox", get(inbox).post(send))
        .route("/api/v1/mailbox/{id}", delete(ack))
        .route("/api/v1/sync/events", get(events))
}

#[derive(Clone, Debug)]
pub struct Note {
    pub event: &'static str,
    pub device: Option<String>,
    pub value: i64,
}

#[derive(Default)]
pub struct Hub {
    channels: Mutex<HashMap<i64, broadcast::Sender<Note>>>,
    live: Mutex<HashSet<String>>,
}

impl Hub {
    pub fn send(&self, user: i64, note: Note) {
        let mut channels = self.channels.lock().unwrap_or_else(PoisonError::into_inner);
        if channels.get(&user).is_some_and(|tx| tx.send(note).is_err()) {
            channels.remove(&user);
        }
    }

    fn subscribe(&self, user: i64) -> broadcast::Receiver<Note> {
        let mut channels = self.channels.lock().unwrap_or_else(PoisonError::into_inner);
        channels.entry(user).or_insert_with(|| broadcast::channel(64).0).subscribe()
    }
}

#[derive(Deserialize)]
struct Message {
    to_device: String,
    kind: String,
    body: String,
}

async fn send(State(app): St, auth: Auth, Input(m): Input<Message>) -> Res<(StatusCode, Json<Value>)> {
    let from = auth.device()?.to_string();
    require(
        ["key-share", "apply", "send", "ping"].contains(&m.kind.as_str()),
        "kind: key-share, apply, send or ping",
    )?;
    let body = B64.decode(&m.body).map_err(|_| Problem::bad("body: base64"))?;
    require(body.len() <= 64 << 10, "body: at most 64 KiB")?;
    let (user, to) = (auth.user, m.to_device.clone());
    let id = app.db.run(move |c| {
        one::<i64>(c, "SELECT 1 FROM devices WHERE id = ?1 AND user_id = ?2", params![to, user])?
            .ok_or_else(|| Problem::missing("device"))?;
        let waiting = "SELECT COUNT(*) FROM mailbox WHERE to_device = ?1 AND expires_at > ?2";
        if one::<i64>(c, waiting, params![to, now()])? >= Some(100) {
            return Err(Problem::conflict("mailbox_full", "the device has 100 unread messages"));
        }
        let sql = "INSERT INTO mailbox (user_id, to_device, from_device, kind, body, created_at, \
                   expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
        c.execute(sql, params![user, to, from, m.kind, body, now(), now() + 7 * DAY])?;
        Ok(c.last_insert_rowid())
    });
    let id = id.await?;
    app.hub.send(auth.user, Note { event: "mail", device: Some(m.to_device), value: id });
    Ok((StatusCode::CREATED, Json(json!({"id": id}))))
}

#[derive(Deserialize)]
struct After {
    #[serde(default)]
    after: i64,
}

async fn inbox(State(app): St, auth: Auth, Query(a): Query<After>) -> Reply {
    let device = auth.device()?.to_string();
    let sql = "SELECT id, from_device, kind, body, created_at FROM mailbox WHERE to_device = ?1 AND id > ?2 \
               AND expires_at > ?3 ORDER BY id LIMIT 100";
    app.db.items(sql, vec![device.into(), a.after.into(), now().into()]).await
}

async fn ack(State(app): St, auth: Auth, Path(id): Path<i64>) -> Res<StatusCode> {
    let device = auth.device()?.to_string();
    app.db
        .run(move |c| {
            changed(
                c.execute("DELETE FROM mailbox WHERE id = ?1 AND to_device = ?2", params![id, device])?,
                "message",
            )
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

struct Live(Arc<App>, String);

impl Drop for Live {
    fn drop(&mut self) {
        self.0.hub.live.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.1);
    }
}

async fn events(State(app): St, auth: Auth) -> Res<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let device = auth.device()?.to_string();
    if !app.hub.live.lock().unwrap_or_else(PoisonError::into_inner).insert(device.clone()) {
        return Err(Problem::conflict("connected", "this device already has an open event stream"));
    }
    let receiver = app.hub.subscribe(auth.user);
    let live = Live(app.clone(), device);
    let stream = futures_util::stream::unfold((receiver, live), |(mut receiver, live)| async move {
        loop {
            match receiver.recv().await {
                Ok(note) if note.device.as_ref().is_none_or(|d| *d == live.1) => {
                    let key = if note.event == "mail" { "id" } else { note.event };
                    let event = Event::default().event(note.event).data(json!({key: note.value}).to_string());
                    return Some((Ok(event), (receiver, live)));
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(25))))
}
