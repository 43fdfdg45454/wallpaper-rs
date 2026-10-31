use crate::auth::Auth;
use crate::db::{changed, now, rows};
use crate::error::{Input, Problem, Res, require};
use crate::net::Client;
use crate::{Reply, St};
use axum::Json;
use axum::extract::State;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct Approve {
    user_code: String,
    #[serde(default = "yes")]
    approve: bool,
    #[serde(default)]
    password: Option<String>,
}

fn yes() -> bool {
    true
}

pub fn user_code(text: &str) -> Res<String> {
    let letters: String = text.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_uppercase();
    require(letters.len() == 8, "user_code: the 8 letters shown on the new device")?;
    Ok(format!("{}-{}", &letters[..4], &letters[4..]))
}

pub fn pending_device(c: &Connection, code: &str) -> Res<Value> {
    let sql = "SELECT platform, client, created_at AS requested_at, expires_at FROM device_codes \
               WHERE user_code = ?1 AND state = 'pending' AND expires_at > ?2";
    rows(c, sql, params![code, now()])?.pop().ok_or_else(|| Problem::missing("pending code"))
}

#[derive(Deserialize)]
pub struct Preview {
    user_code: String,
}

pub async fn preview(State(app): St, auth: Auth, Input(p): Input<Preview>) -> Reply {
    if auth.device.is_some() { auth.scope("sync") } else { auth.browser() }?;
    let code = user_code(&p.user_code)?;
    Ok(Json(app.db.run(move |c| pending_device(c, &code)).await?))
}

pub async fn approve(State(app): St, auth: Auth, client: Client, Input(a): Input<Approve>) -> Reply {
    match (&auth.device, a.password.clone()) {
        (None, _) => auth.browser()?,
        (Some(_), Some(password)) => {
            let (who, user) = (auth.handle.clone(), auth.user);
            let checked = crate::account::authenticate(&app, &client, &who, password).await?;
            require(checked == user, "the password belongs to another account")?;
        }
        (Some(_), None) => {
            return Err(Problem::forbidden(
                "approving a device needs the account password, or the account page of the web site",
            ));
        }
    }
    let code = user_code(&a.user_code)?;
    let state = if a.approve { "approved" } else { "denied" };
    let sql = "UPDATE device_codes SET state = ?2, user_id = ?3 WHERE user_code = ?1 AND state = 'pending' \
               AND expires_at > ?4";
    app.db
        .run(move |c| changed(c.execute(sql, params![code, state, auth.user, now()])?, "pending code"))
        .await?;
    Ok(Json(json!({"approved": a.approve})))
}
