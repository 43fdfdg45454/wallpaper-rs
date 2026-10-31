use super::{CHUNK, device, dir, held, row};
use crate::auth::Auth;
use crate::db::changed;
use crate::error::{Problem, Res, require};
use crate::{Reply, St};
use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use futures_util::StreamExt;
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

#[derive(Deserialize)]
pub(super) struct At {
    #[serde(default)]
    offset: i64,
    length: Option<i64>,
}

pub(super) async fn append(
    State(app): St,
    auth: Auth,
    Path(id): Path<String>,
    Query(at): Query<At>,
    headers: HeaderMap,
    body: Body,
) -> Reply {
    let user = device(&auth, &id)?;
    let length = headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()?.parse::<i64>().ok());
    let length = length.filter(|n| (1..=CHUNK).contains(n)).ok_or_else(|| {
        Problem::new(StatusCode::LENGTH_REQUIRED, "length", "send a Content-Length of 1 byte to 8 MiB")
    })?;
    let _turn = app.pieces.enter(user, &id)?;
    let (size, received) = row(&app, user, &id).await?;
    let received = held(&app, user, &id, received).await?;
    if at.offset != received {
        return Err(Problem::conflict("offset", format!("the server holds {received} bytes")));
    }
    require(received + length <= size, "the data goes past the declared size")?;
    let folder = dir(&app, user);
    tokio::fs::DirBuilder::new().recursive(true).mode(0o700).create(&folder).await?;
    let mut file =
        tokio::fs::OpenOptions::new().create(true).write(true).truncate(false).open(folder.join(&id)).await?;
    file.set_len(received as u64).await?;
    file.seek(SeekFrom::Start(received as u64)).await?;
    let copy = async {
        let (mut stream, mut total) = (body.into_data_stream(), 0);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| Problem::bad(format!("upload interrupted: {e}")))?;
            total += chunk.len() as i64;
            require(total <= length, "the body is longer than its Content-Length")?;
            file.write_all(&chunk).await?;
        }
        require(total == length, "the body is shorter than its Content-Length")?;
        Ok::<_, Problem>(file.sync_data().await?)
    };
    let late = |_| Problem::bad("the piece took longer than 2 minutes");
    tokio::time::timeout(Duration::from_secs(120), copy).await.map_err(late)??;
    let (key, done) = (id.clone(), received + length);
    let sql = "UPDATE sync_blobs SET received = ?3 WHERE user_id = ?1 AND id = ?2 AND received = ?4";
    let moved = app.db.run(move |c| Ok(c.execute(sql, params![user, key, done, received])?)).await?;
    if moved == 0 {
        return Err(Problem::conflict("offset", "the file changed while this piece arrived"));
    }
    Ok(Json(json!({"id": id, "size": size, "received": done})))
}

pub(super) async fn read(
    State(app): St,
    auth: Auth,
    Path(id): Path<String>,
    Query(at): Query<At>,
) -> Res<impl IntoResponse> {
    let user = device(&auth, &id)?;
    let (size, received) = row(&app, user, &id).await?;
    if received < size || held(&app, user, &id, received).await? < size {
        return Err(Problem::conflict("incomplete", "the file is still being uploaded"));
    }
    require((0..=size).contains(&at.offset), "offset: inside the file")?;
    let length = at.length.unwrap_or(CHUNK).clamp(0, CHUNK).min(size - at.offset);
    let path = dir(&app, user).join(&id);
    let bytes = tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path)?;
        file.seek(SeekFrom::Start(at.offset as u64))?;
        let mut bytes = vec![0; length as usize];
        file.read_exact(&mut bytes).map(|()| bytes)
    });
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes.await??))
}

pub(super) async fn remove(State(app): St, auth: Auth, Path(id): Path<String>) -> Res<StatusCode> {
    let user = device(&auth, &id)?;
    let path = dir(&app, user).join(&id);
    let sql = "DELETE FROM sync_blobs WHERE user_id = ?1 AND id = ?2";
    app.db.run(move |c| changed(c.execute(sql, params![user, id])?, "file")).await?;
    let _ = tokio::fs::remove_file(&path).await;
    Ok(StatusCode::NO_CONTENT)
}
