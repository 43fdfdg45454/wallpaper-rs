use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, invalid};
use serde_json::json;

impl Daemon {
    pub fn vote(&self, id: &str, vote: &str) -> Result<String> {
        if !wprs_format::valid_id(id) || !matches!(vote, "up" | "down" | "none") {
            return Err(invalid("give a package id and up, down or none"));
        }
        let (api, _) = self.account_api(true)?;
        let body = json!({ "vote": vote });
        let found = block_on(api.post(&format!("/api/v1/packages/{id}/vote"), body)).map_err(sync_error)?;
        Ok(found.to_string())
    }

    pub fn votes(&self, ids: &[String]) -> Result<String> {
        if ids.len() > 100 || !ids.iter().all(|i| wprs_format::valid_id(i)) {
            return Err(invalid("give at most 100 package ids"));
        }
        let none = json!({ "items": {}, "shown": false }).to_string();
        if ids.is_empty() || !self.state().config.repos.vote_counts || self.saved().url.is_empty() {
            return Ok(none);
        }
        let Ok((api, _)) = self.account_api(false) else { return Ok(none) };
        let found = block_on(api.get(&format!("/api/v1/votes?ids={}", ids.join(",")))).map_err(sync_error)?;
        Ok(json!({ "items": found["items"], "shown": true }).to_string())
    }
}
