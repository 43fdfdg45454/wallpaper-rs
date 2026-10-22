use super::views::Local;
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed};
use crate::jobs::now;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use wprs_core::model::JobKind;
use wprs_sync::{Replica, Rules, Session, View, now_ms};

pub fn rules(collection: &str) -> Rules {
    match collection {
        "library" => Rules { weak: |key| key == "title", keep_absent: true, needs_strong: true },
        "settings" => Rules { weak: |key| key.contains('.'), ..Rules::default() },
        "playlists" => Rules::default(),
        _ => Rules { keep_absent: true, ..Rules::default() },
    }
}

fn arrived(
    local: &Local,
    replica: &Replica,
    applied: &[(String, Option<View>)],
) -> Vec<(String, Option<View>)> {
    let mentions =
        |view: &View| view.values().any(|v| local.fresh.iter().any(|k| v.to_string().contains(k.as_str())));
    let fresh = |key: &str| key.strip_prefix("library/").is_some_and(|k| local.fresh.contains(k));
    let views = replica.bases.iter().filter(|(key, _)| !applied.iter().any(|(k, _)| k == *key));
    let views = views.filter_map(|(key, (_, doc))| Some((key, doc.view()?)));
    let wanted = |key: &str, view: &View| fresh(key) || local.unlinked.contains(key) || mentions(view);
    views.filter(|(key, view)| wanted(key, view)).map(|(key, view)| (key.clone(), Some(view))).collect()
}

impl Daemon {
    pub fn sync_now(self: &Arc<Self>) -> Result<u64> {
        self.require_ready()?;
        Ok(self.account_sync_job(true))
    }

    pub fn account_sync_job(self: &Arc<Self>, prompt: bool) -> u64 {
        self.start_job(JobKind::AccountSync, "", move |d, _| {
            let result = d.cycle(prompt);
            let error = result.as_ref().err().map(ToString::to_string);
            d.update(|s| {
                s.last_sync = if error.is_none() { Some(now()) } else { s.last_sync };
                s.last_error = error;
            })?;
            result
        })
    }

    fn cycle(self: &Arc<Self>, prompt: bool) -> Result<Value> {
        self.require_ready()?;
        let (api, secrets) = self.account_api(prompt)?;
        let state = block_on(api.get("/api/v1/sync/state")).map_err(sync_error)?;
        let (encrypted, epoch) =
            (self.saved().encrypted.unwrap_or(true), state["epoch"].as_i64().unwrap_or(1));
        if state["encrypted"].as_bool() != Some(encrypted) {
            return Err(failed("the server changed the encryption of this account; nothing was synced"));
        }
        let (owner, user, device, label) = {
            let s = self.saved();
            (s.owner.clone(), s.user(), s.device.clone().unwrap_or_default(), s.label.clone())
        };
        let key = secrets.sync_key()?;
        let ids = key.ids();
        let session = Session { api: api.clone(), user, epoch, encrypted, key, rejected: Default::default() };
        let (mut replica, mut links, mut clock, present) = {
            let s = self.saved();
            (s.replica.clone(), s.links.clone(), s.clock.clone(), s.present.clone())
        };
        let mut local = self.snapshot(&ids, &replica, &mut links, &present)?;
        if encrypted {
            let me = View::from([("name".to_owned(), json!(label)), ("platform".to_owned(), json!("linux"))]);
            local.views.insert(format!("device/{device}"), me);
        }
        let now = clock.tick(now_ms(), &device);
        let applied = block_on(wprs_sync::sync(&session, &mut replica, &local.views, rules, &now));
        let mut applied = applied.map_err(sync_error)?;
        applied.extend(arrived(&local, &replica, &applied));
        let applied = self.apply_remote(&local, applied, &mut links);
        replica.bases.values().for_each(|(_, doc)| clock.observe(&doc.clock()));
        let rejected = session.rejected.load(Ordering::Relaxed);
        let records = replica.bases.len();
        let _ = self.refresh_peers(&api);
        self.update(|s| {
            if s.owner != owner {
                return;
            }
            (s.replica, s.links, s.clock) = (replica, links, clock);
            s.present = local.items.into_keys().collect();
            s.epoch = epoch;
            s.warning = state["warning"].as_str().map(String::from);
            (s.used_bytes, s.quota_bytes) =
                (state["used_bytes"].as_u64().unwrap_or(0), state["quota_bytes"].as_u64().unwrap_or(0));
        })?;
        if rejected > 0 {
            return Err(failed(format!("{rejected} records could not be decrypted with this sync key")));
        }
        Ok(json!({"applied": applied, "records": records}))
    }
}
