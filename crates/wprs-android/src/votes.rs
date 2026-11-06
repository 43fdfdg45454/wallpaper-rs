use crate::state::{api, linked};
use crate::{Error, said, server, settings};
use serde_json::{Map, Value, json};
use wprs_format::voted;

fn runtime() -> Result<tokio::runtime::Runtime, Error> {
    Ok(tokio::runtime::Builder::new_current_thread().enable_all().build()?)
}

fn answer(path: String, body: Option<Value>) -> Result<Value, Error> {
    let (api, _) = api()?;
    Ok(runtime()?.block_on(async {
        match body {
            Some(body) => api.post(&path, body).await,
            None => api.get(&path).await,
        }
    })?)
}

fn repository() -> Result<Option<String>, Error> {
    linked()?;
    runtime()?.block_on(server::repository())
}

pub(crate) fn counts(ids: &[String]) -> Result<Value, Error> {
    if voted(ids, None).is_none() {
        return Err(said("android.vote_ids", "give at most 1000 package ids", &[]));
    }
    let none = |repo: &Option<String>| json!({"items": {}, "shown": false, "repo_id": repo});
    if ids.is_empty() || linked().is_err() {
        return Ok(none(&None));
    }
    let repo = repository()?;
    let asked = voted(ids, repo.as_deref()).unwrap_or_default();
    if asked.is_empty() || !settings::config().repos.vote_counts {
        return Ok(none(&repo));
    }
    let mut found = Map::new();
    for query in wprs_format::vote_queries(&asked) {
        found.extend(answer(query, None)?["items"].as_object().cloned().unwrap_or_default());
    }
    let count = |id: &str| found.get(id).cloned().unwrap_or(Value::Null);
    let items: Map<String, Value> = asked
        .iter()
        .filter(|(_, id)| !count(id).is_null())
        .map(|(spec, id)| ((*spec).into(), count(id)))
        .collect();
    Ok(json!({"items": items, "shown": true, "repo_id": repo}))
}

pub(crate) fn cast(id: &str, vote: &str) -> Result<Value, Error> {
    let specs = [id.to_owned()];
    if voted(&specs, None).is_none() || !matches!(vote, "up" | "down" | "none") {
        return Err(said("android.vote_value", "give a package id and up, down or none", &[]));
    }
    let repo = repository()?;
    let Some((_, id)) = voted(&specs, repo.as_deref()).unwrap_or_default().first().copied() else {
        return Err(said(
            "android.vote_elsewhere",
            "votes go only to packages of the repository of the linked server",
            &[],
        ));
    };
    answer(format!("/api/v1/packages/{id}/vote"), Some(json!({ "vote": vote })))
}
