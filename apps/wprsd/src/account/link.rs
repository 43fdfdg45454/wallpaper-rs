use super::secrets::{Secrets, encode};
use super::server::{exists, pause, same_origin};
use super::{block_on, hostname, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid};
use crate::jobs::Ctl;
use reqwest::Method;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_core::dbus::error::EXISTS;
use wprs_core::model::JobKind;
use wprs_sync::{AGENT, Api, DeviceKey, Poll, SyncKey, poll};

impl Daemon {
    pub fn link_device(self: &Arc<Self>, name: &str, plaintext: bool) -> Result<(String, String, u64)> {
        let url = self.server_url()?;
        if self.saved().device.is_some() || !self.jobs.find(JobKind::Link, "link").is_empty() {
            return Err(exists("this computer is already linked or linking"));
        }
        let label = Some(name.trim()).filter(|n| !n.is_empty()).map_or_else(hostname, String::from);
        if label.chars().count() > 64 || label.chars().any(char::is_control) {
            return Err(invalid("a device name has 1 to 64 printable characters"));
        }
        let (key, api) = (DeviceKey::generate(), Api::new(&url).map_err(sync_error)?);
        let body = json!({"platform": "linux", "enc_key": encode(&key.public()), "client": AGENT});
        let start = block_on(api.post("/api/v1/auth/device/start", body)).map_err(sync_error)?;
        let text = |field: &str| start[field].as_str().unwrap_or_default().to_owned();
        let (code, interval, expires) =
            (text("device_code"), start["interval"].as_u64(), start["expires_in"].as_u64());
        let (interval, expires) = (interval.unwrap_or(5).clamp(1, 60), expires.unwrap_or(600).min(1800));
        let job = self.start_job(JobKind::Link, "link", move |d, ctl| {
            let token = d.enroll(&api, &code, interval, expires, ctl)?;
            d.finish_link(api, token, key, label, plaintext)
        });
        let shown = Some(text("verification_uri")).filter(|uri| same_origin(uri, &url));
        Ok((text("user_code"), shown.unwrap_or_else(|| format!("{url}/link")), job))
    }

    fn enroll(&self, api: &Api, code: &str, mut interval: u64, expires: u64, ctl: &Ctl) -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(expires);
        while Instant::now() < deadline {
            pause(ctl, Duration::from_secs(interval))?;
            let answer = block_on(api.post("/api/v1/auth/device/token", json!({"device_code": code})));
            match poll(&mut interval, answer) {
                Poll::Done(token) => return Ok(token),
                Poll::Wait(_) => {}
                Poll::Stop(e) if e.code() == "access_denied" => return Err(failed("the link was refused")),
                Poll::Stop(e) => return Err(sync_error(e)),
            }
        }
        Err(failed("the code expired before it was approved; link again"))
    }

    fn finish_link(
        self: &Arc<Self>,
        api: Api,
        token: Value,
        key: DeviceKey,
        label: String,
        plaintext: bool,
    ) -> Result<Value> {
        let text = |field: &str| token[field].as_str().unwrap_or_default().to_owned();
        let mut secrets = Secrets::default();
        (secrets.token, secrets.device) = (text("token"), encode(&key.bytes()));
        self.store_secrets(&secrets)?;
        let api = api.with_token(&secrets.token);
        let me = block_on(api.get("/api/v1/me")).map_err(sync_error)?;
        let existing = match block_on(api.get("/api/v1/sync/state")) {
            Err(e) if e.code() == "sync_not_set_up" => None,
            state => Some(state.map_err(sync_error)?),
        };
        if existing.as_ref().is_some_and(|state| state["encrypted"] != true) && !plaintext {
            let gone = format!("/api/v1/me/devices/{}", text("device_id"));
            let _ = block_on(api.call(Method::DELETE, &gone, None));
            self.forget_secrets()?;
            return Err(failed(
                "the account syncs without end-to-end encryption; link with plain text only if you accept that",
            ));
        }
        let owner = (self.server_url()?, me["id"].as_i64().unwrap_or_default());
        self.update(|s| {
            if s.owner.as_ref() != Some(&owner) {
                (s.replica, s.links, s.clock, s.present) = Default::default();
                (s.kept, s.declined, s.unapplied, s.offers) = Default::default();
                (s.files.stored, s.files.dropped, s.files.uploaded) = Default::default();
                s.owner = Some(owner.clone());
            }
            (s.device, s.device_name, s.label) = (Some(text("device_id")), Some(text("device_name")), label);
            (s.plaintext, s.pinned, s.members) = (plaintext, Some(Default::default()), Default::default());
            s.handle = me["handle"].as_str().map(String::from);
        })?;
        let state = match existing {
            None => {
                let sync = SyncKey::generate();
                secrets.set_key(&sync, 0);
                self.store_secrets(&secrets)?;
                let body =
                    json!({"encrypted": !plaintext, "confirm_plaintext": plaintext, "key_id": sync.id()});
                let state = block_on(api.post("/api/v1/sync/setup", body)).map_err(sync_error)?;
                if state["encrypted"].as_bool() != Some(!plaintext) {
                    return Err(failed("the server set up sync with another encryption than asked"));
                }
                secrets.set_key(&sync, state["epoch"].as_i64().unwrap_or(0));
                self.store_secrets(&secrets)?;
                state
            }
            Some(state) => state,
        };
        self.update(|s| {
            (s.encrypted, s.epoch) = (state["encrypted"].as_bool(), state["epoch"].as_i64().unwrap_or(0));
            (s.warning, s.ready, s.stale) =
                (state["warning"].as_str().map(String::from), !secrets.key.is_empty(), false);
        })?;
        let repo = self.saved().repo_url.clone();
        if let Some(Err(e)) =
            repo.map(|url| self.add_repo(&url)).filter(|r| r.as_ref().is_err_and(|e| e.parts().0 != EXISTS))
        {
            tracing::warn!("the repository of the server could not be added: {e}");
        }
        let _ = self.refresh_peers(&api);
        self.account.wake.notify_one();
        self.account.restart.notify_one();
        Ok(
            json!({"device": text("device_name"), "encrypted": state["encrypted"], "needs_key": secrets.key.is_empty()}),
        )
    }
}
