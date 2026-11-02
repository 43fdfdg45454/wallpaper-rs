use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, invalid};
use serde_json::{Map, Value, json};

const ELSEWHERE: &str = "votes go only to packages of the repository of the linked server";

impl Daemon {
    fn server_repo(&self) -> Option<String> {
        let saved = self.saved();
        saved.repo_id.clone().filter(|_| !saved.url.is_empty())
    }

    pub fn vote(&self, spec: &str, vote: &str) -> Result<String> {
        let specs = [spec.to_owned()];
        let found = wprs_format::voted(&specs, self.server_repo().as_deref());
        let Some(asked) = found.filter(|_| matches!(vote, "up" | "down" | "none")) else {
            return Err(invalid("give a package id or REPO/ID and up, down or none"));
        };
        let Some((_, id)) = asked.first() else { return Err(invalid(ELSEWHERE)) };
        let (api, _) = self.account_api(true)?;
        let body = json!({ "vote": vote });
        let found = block_on(api.post(&format!("/api/v1/packages/{id}/vote"), body)).map_err(sync_error)?;
        Ok(found.to_string())
    }

    pub fn votes(&self, specs: &[String]) -> Result<String> {
        let server = self.server_repo();
        let Some(asked) = wprs_format::voted(specs, server.as_deref()) else {
            return Err(invalid("give at most 100 package ids, each as ID or REPO/ID"));
        };
        let none = json!({ "items": {}, "shown": false, "repo_id": server }).to_string();
        if asked.is_empty() || !self.state().config.repos.vote_counts || self.saved().url.is_empty() {
            return Ok(none);
        }
        let Ok((api, _)) = self.account_api(false) else { return Ok(none) };
        let mut ids: Vec<&str> = asked.iter().map(|(_, id)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        let found = block_on(api.get(&format!("/api/v1/votes?ids={}", ids.join(",")))).map_err(sync_error)?;
        let count = |id: &str| found["items"].get(id).cloned().unwrap_or(Value::Null);
        let items: Map<String, Value> = asked
            .iter()
            .filter(|(_, id)| !count(id).is_null())
            .map(|(spec, id)| ((*spec).into(), count(id)))
            .collect();
        Ok(json!({ "items": items, "shown": true, "repo_id": server }).to_string())
    }
}
