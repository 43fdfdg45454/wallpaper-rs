use super::mail::{self, post, wait};
use super::saved::{Pair, Peer};
use super::server::pause;
use super::{block_on, sync_error};
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid, not_found};
use crate::jobs::Ctl;
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_core::model::JobKind;
use wprs_sync::{Api, DeviceKey, Mail, aad, code, commit, hex, open_key, random, transcript};

pub fn find(peers: &[Peer], wanted: &str, labels: impl Fn(&str) -> Option<String>) -> Result<Peer> {
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

pub fn nonce(mail: &Mail) -> Result<[u8; 32]> {
    mail.bytes().map_err(sync_error)?.try_into().map_err(|_| failed("a device message is broken"))
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

    pub fn join(self: &Arc<Self>, device: &str) -> Result<u64> {
        self.linked()?;
        if self.saved().ready {
            return Err(invalid("this computer already holds the sync key"));
        }
        let (api, secrets) = self.account_api(true)?;
        let (mine, peer) = (secrets.device_key()?, self.peer(&api, device)?);
        public(&peer)?;
        Ok(self.start_job(JobKind::Link, &format!("join {}", peer.id), move |d, ctl| {
            let received = d.receive_key(&api, &mine, &peer, ctl);
            d.update(|s| s.pairing = None)?;
            received
        }))
    }

    fn receive_key(&self, api: &Api, mine: &DeviceKey, peer: &Peer, ctl: &Ctl) -> Result<Value> {
        let state = block_on(api.get("/api/v1/sync/state")).map_err(sync_error)?;
        let (user, me, encrypted) = {
            let s = self.saved();
            (s.user(), s.device.clone().unwrap_or_default(), s.encrypted.unwrap_or(true))
        };
        let (epoch, theirs) = (state["epoch"].as_i64().unwrap_or(0), public(peer)?);
        mail::from(api, &peer.id)?;
        lock(&self.account.confirmed).take();
        let (pair, mine_nonce) = (hex(&random::<16>()), random::<32>());
        post(api, &peer.id, &Mail::body("commit", &pair, &commit(pair.as_bytes(), &mine_nonce), 0))?;
        let pending = Pair { device: peer.id.clone(), pair: pair.clone(), ..Pair::default() };
        self.update(|s| s.pairing = Some(pending.clone()))?;
        let theirs_nonce = nonce(&wait(api, peer, &pair, "nonce", ctl)?)?;
        post(api, &peer.id, &Mail::body("reveal", &pair, &mine_nonce, 0))?;
        let bound = transcript(pair.as_bytes(), (&mine.public(), &mine_nonce), (&theirs, &theirs_nonce));
        let shown = code(&bound);
        self.update(|s| s.pairing = Some(Pair { code: Some(shown.clone()), ..pending }))?;
        let sealed = self.confirmed(api, peer, &pair, &shown.replace(' ', ""), ctl)?;
        let opened = open_key(mine, &theirs, &aad(user, &me, epoch, encrypted, &bound), &sealed);
        let key = opened.map_err(|_| {
            failed(format!(
                "the key from {} does not open here: the server changed keys or settings",
                peer.name
            ))
        })?;
        if state["key_id"].as_str().is_some_and(|id| id != key.id()) {
            return Err(failed(format!(
                "{} sent a key that is not the current key of the account",
                peer.name
            )));
        }
        let mut secrets = self.secrets(true)?;
        secrets.set_key(&key, epoch);
        self.store_secrets(&secrets)?;
        self.pin(&peer.id, peer.enc_key.as_deref().unwrap_or_default())?;
        self.rekeyed(api)?;
        self.update(|s| {
            (s.ready, s.stale, s.last_error, s.epoch) = (true, false, None, epoch);
            (s.replica.seq, s.quarantine, s.unapplied) = Default::default();
        })?;
        self.account.wake.notify_one();
        self.account.restart.notify_one();
        Ok(json!({"from": peer.name}))
    }

    fn confirmed(&self, api: &Api, peer: &Peer, pair: &str, shown: &str, ctl: &Ctl) -> Result<Vec<u8>> {
        let deadline = Instant::now() + Duration::from_secs(600);
        let (mut sealed, mut typed) = (None, None);
        while Instant::now() < deadline {
            if typed.is_none() {
                let mut slot = lock(&self.account.confirmed);
                typed = slot.take_if(|(device, _)| *device == peer.id).map(|(_, digits)| digits);
            }
            if typed.as_ref().is_some_and(|digits| digits != shown) {
                post(api, &peer.id, &Mail::body("abort", pair, &[], 0))?;
                return Err(invalid(
                    "the codes differ: the key was not taken; someone may stand between the devices",
                ));
            }
            if sealed.is_none() {
                sealed =
                    mail::next(api, peer, pair, "key")?.map(|m| m.bytes().map_err(sync_error)).transpose()?;
            }
            if let (Some(sealed), Some(_)) = (&sealed, &typed) {
                return Ok(sealed.clone());
            }
            pause(ctl, Duration::from_secs(1))?;
        }
        Err(failed(format!("the codes were not confirmed with {} within 10 minutes; start again", peer.name)))
    }
}
