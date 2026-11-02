use crate::state::{api, linked};
use crate::{Error, said, settings};
use serde_json::{Value, json};
use wprs_format::valid_id;

const MOST: usize = 100;

fn answer(path: String, body: Option<Value>) -> Result<Value, Error> {
    let (api, _) = api()?;
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    Ok(runtime.block_on(async {
        match body {
            Some(body) => api.post(&path, body).await,
            None => api.get(&path).await,
        }
    })?)
}

pub(crate) fn counts(ids: &[String]) -> Result<Value, Error> {
    if ids.len() > MOST || !ids.iter().all(|i| valid_id(i)) {
        return Err(said("android.vote_ids", "give at most 100 package ids", &[]));
    }
    if ids.is_empty() || !settings::config().repos.vote_counts || linked().is_err() {
        return Ok(json!({"items": {}, "shown": false}));
    }
    let found = answer(format!("/api/v1/votes?ids={}", ids.join(",")), None)?;
    Ok(json!({"items": found["items"], "shown": true}))
}

pub(crate) fn cast(id: &str, vote: &str) -> Result<Value, Error> {
    if !valid_id(id) || !matches!(vote, "up" | "down" | "none") {
        return Err(said("android.vote_value", "give a package id and up, down or none", &[]));
    }
    linked()?;
    answer(format!("/api/v1/packages/{id}/vote"), Some(json!({ "vote": vote })))
}
