use super::saved::Peer;
use super::server::pause;
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid, not_found};
use crate::jobs::Ctl;
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_core::model::JobKind;
use wprs_sync::{Api, DeviceKey, code, open_key};

fn find(peers: &[Peer], wanted: &str, labels: impl Fn(&str) -> Option<String>) -> Result<Peer> {
    let wanted = wanted.trim();
    let matches = |p: &&Peer| {
        p.id == wanted || p.name.eq_ignore_ascii_case(wanted) || labels(&p.id).is_some_and(|l| l == wanted)
    };
    let peer =
        peers.iter().find(matches).ok_or_else(|| not_found(format!("no device {wanted} on the server")))?;
    Ok(peer.clone())
}

pub fn public(peer: &Peer) -> Result<Vec<u8>> {
    let key = peer.enc_key.as_deref().and_then(|k| B64.decode(k).ok());
    key.ok_or_else(|| invalid(format!("{} has no encryption key to share with", peer.name)))
}

impl Daemon {
    pub fn peer(&self, api: &Api, device: &str) -> Result<Peer> {
        let peers = self.refresh_peers(api)?;
        let me = self.saved().device.clone();
        let peer = find(&peers, device, |id| self.saved().label_of(id))?;
        if Some(&peer.id) == me.as_ref() {
            Err(invalid("choose another device than this one"))
        } else {
            Ok(peer)
        }
    }

    pub fn aad(&self, api: &Api, device: &str) -> Result<Vec<u8>> {
        let state = block_on(api.get("/api/v1/sync/state")).map_err(sync_error)?;
        let (user, sealed) = {
            let s = self.saved();
            (s.user(), s.encrypted.unwrap_or(true))
        };
        let epoch = state["epoch"].as_i64().unwrap_or(1);
        Ok([&user.to_be_bytes(), device.as_bytes(), &epoch.to_be_bytes(), &[u8::from(sealed)]].concat())
    }

    pub fn join(self: &Arc<Self>, device: &str) -> Result<(String, u64)> {
        self.linked()?;
        if self.saved().ready {
            return Err(invalid("this computer already holds the sync key"));
        }
        let (api, secrets) = self.account_api(true)?;
        let (mine, peer) = (secrets.device_key()?, self.peer(&api, device)?);
        let shown = code(&mine.public(), &public(&peer)?);
        let job = self.start_job(JobKind::Link, &format!("join {}", peer.id), move |d, ctl| {
            d.receive_key(&api, mine, &peer, ctl)
        });
        Ok((shown, job))
    }

    fn receive_key(&self, api: &Api, mine: DeviceKey, peer: &Peer, ctl: &Ctl) -> Result<Value> {
        let me = self.saved().device.clone().unwrap_or_default();
        let (aad, theirs, deadline) =
            (self.aad(api, &me)?, public(peer)?, Instant::now() + Duration::from_secs(600));
        while Instant::now() < deadline {
            let mail = block_on(api.get("/api/v1/mailbox")).map_err(sync_error)?;
            let mut shares =
                mail["items"].as_array().into_iter().flatten().filter(|m| m["kind"] == "key-share");
            if let Some(message) = shares.find(|m| m["from_device"] == peer.id.as_str()) {
                let body = B64.decode(message["body"].as_str().unwrap_or_default()).unwrap_or_default();
                let opened = open_key(&mine, &theirs, &aad, &body);
                let path = format!("/api/v1/mailbox/{}", message["id"]);
                block_on(api.call(reqwest::Method::DELETE, &path, None)).map_err(sync_error)?;
                let Ok(key) = opened else {
                    return Err(failed(format!(
                        "the key from {} does not open here: the server changed the keys or the encryption",
                        peer.name
                    )));
                };
                let mut secrets = self.secrets(true)?;
                secrets.key = super::secrets::encode(key.bytes());
                self.store_secrets(&secrets)?;
                self.update(|s| (s.ready, s.last_error) = (true, None))?;
                self.account.wake.notify_one();
                self.account.restart.notify_one();
                return Ok(json!({"from": peer.name}));
            }
            pause(ctl, Duration::from_secs(3))?;
        }
        Err(failed(format!("no key arrived from {} within 10 minutes; join again", peer.name)))
    }
}
