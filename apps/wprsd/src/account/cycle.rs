use super::secrets::Secrets;
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed};
use crate::jobs::now;
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_core::model::JobKind;
use wprs_sync::Api;

mod turn;

impl Daemon {
    pub fn sync_now(self: &Arc<Self>) -> Result<u64> {
        self.linked()?;
        let keyed = {
            let s = self.saved();
            s.ready || s.stale
        };
        if !keyed {
            return Err(failed("this computer has no sync key yet; join from a linked device"));
        }
        Ok(self.account_sync_job(true))
    }

    pub fn account_sync_job(self: &Arc<Self>, prompt: bool) -> u64 {
        self.start_job(JobKind::AccountSync, "", move |d, _| {
            let result = d.cycle(prompt);
            d.retry_later(result.as_ref().map_or(None, |v| v["unsent"].as_u64()));
            let error = result.as_ref().err().map(ToString::to_string);
            d.update(|s| {
                s.last_sync = if error.is_none() { Some(now()) } else { s.last_sync };
                s.last_error = error;
            })?;
            result
        })
    }

    fn current_key(&self, api: &Api, secrets: &mut Secrets) -> Result<Value> {
        let get = || block_on(api.get("/api/v1/sync/state")).map_err(sync_error);
        let (mut state, encrypted) = (get()?, self.saved().encrypted.unwrap_or(true));
        if state["encrypted"].as_bool() != Some(encrypted) {
            return Err(failed("the server changed the encryption of this account; nothing was synced"));
        }
        if secrets.key.is_empty() {
            return Err(failed("this computer has no sync key yet; join from a linked device"));
        }
        if encrypted && !secrets.next.is_empty() {
            self.rotate(api, secrets, state["rotate"] == true)?;
            state = get()?;
        }
        let (server, id) = (state["epoch"].as_i64().unwrap_or_default(), state["key_id"].as_str());
        let (mut epoch, mut adopted) = (secrets.epoch.unwrap_or(self.saved().epoch), None);
        while encrypted && server > epoch {
            let Some(found) = self.adopt(api, secrets, epoch + 1, id.filter(|_| epoch + 1 == server))? else {
                break;
            };
            (epoch, adopted) = (epoch + 1, Some(found));
        }
        let key = secrets.sync_key()?;
        if server != epoch || id.is_some_and(|id| id != key.id()) {
            self.update(|s| (s.ready, s.stale) = (false, true))?;
            return Err(failed(
                "the sync key of the account changed: waiting for the new key, or join again",
            ));
        }
        if let Some((from, bound)) = adopted {
            if self.rekeyed(api)? {
                secrets.lock.clear();
                self.store_secrets(secrets)?;
            }
            let others = self.pins()?.into_keys().filter(|id| *id != from).collect();
            self.hand_over(api, secrets, epoch, &*bound, Some(others))?;
        }
        if id.is_none() {
            let adopted = json!({"epoch": server, "key_id": key.id()});
            block_on(api.post("/api/v1/sync/epoch", adopted)).map_err(sync_error)?;
        }
        self.update(|s| (s.ready, s.stale, s.epoch) = (true, false, server))?;
        self.resume_hand_over(api, secrets)?;
        Ok(state)
    }
}
