use super::keys::{find, nonce, public};
use super::mail::{self, post, wait};
use super::saved::Pair;
use super::server::pause;
use super::sync_error;
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid};
use crate::jobs::{Ctl, now};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_core::model::JobKind;
use wprs_sync::{Mail, aad, code, commit, random, seal_key, transcript};

impl Daemon {
    pub fn share_key(self: &Arc<Self>, device: &str) -> Result<u64> {
        self.require_ready()?;
        let device = device.to_owned();
        Ok(self.start_job(JobKind::Link, &format!("share {device}"), move |d, ctl| d.compare(&device, ctl)))
    }

    fn compare(&self, device: &str, ctl: &Ctl) -> Result<Value> {
        let (api, secrets) = self.account_api(true)?;
        let (mine, peer) = (secrets.device_key()?, self.peer(&api, device)?);
        let theirs = public(&peer)?;
        let deadline = Instant::now() + Duration::from_secs(600);
        let opened = loop {
            if let Some(letter) = mail::from(&api, &peer.id)?.pop() {
                break letter.mail.map_err(sync_error)?;
            }
            if Instant::now() > deadline {
                return Err(failed(format!("{} did not ask for the key within 10 minutes", peer.name)));
            }
            pause(ctl, Duration::from_secs(2))?;
        };
        if opened.step != "commit" || opened.pair.len() != 32 {
            return Err(failed(format!(
                "{} sent {} instead of a request: start again",
                peer.name, opened.step
            )));
        }
        let (pair, mine_nonce) = (opened.pair.clone(), random::<32>());
        post(&api, &peer.id, &Mail::body("nonce", &pair, &mine_nonce, 0))?;
        let theirs_nonce = nonce(&wait(&api, &peer, &pair, "reveal", ctl)?)?;
        if opened.bytes().map_err(sync_error)? != commit(pair.as_bytes(), &theirs_nonce) {
            post(&api, &peer.id, &Mail::body("abort", &pair, &[], 0))?;
            return Err(failed(
                "the joining device broke its commitment: someone stands between the devices",
            ));
        }
        let bound = transcript(pair.as_bytes(), (&theirs, &theirs_nonce), (&mine.public(), &mine_nonce));
        let (shown, transcript) = (code(&bound), B64.encode(bound));
        let pending = Pair {
            device: peer.id,
            code: Some(shown.clone()),
            sharing: true,
            pair,
            transcript,
            key: B64.encode(&theirs),
            expires: now() + 600,
        };
        self.update(|s| s.pairing = Some(pending))?;
        Ok(json!({"device": peer.name, "code": shown}))
    }

    pub fn confirm_share(self: &Arc<Self>, device: &str, shown: &str) -> Result<u64> {
        let digits: String = shown.chars().filter(char::is_ascii_digit).collect();
        if digits.len() != 6 {
            return Err(invalid("the code has 6 digits"));
        }
        self.linked()?;
        let peers = self.saved().peers.clone();
        let peer = find(&peers, device, |id| self.saved().label_of(id))?;
        let pairing = self.saved().pairing.clone().filter(|p| p.device == peer.id);
        if pairing.as_ref().is_some_and(|p| !p.sharing) {
            let join = self.jobs.find(JobKind::Link, &format!("join {}", peer.id)).into_iter().min();
            let join = join.ok_or_else(|| failed(format!("no comparison with {} is waiting", peer.name)))?;
            *lock(&self.account.confirmed) = Some((peer.id, digits));
            return Ok(join);
        }
        self.require_ready()?;
        let pending = pairing.filter(|p| p.sharing && p.expires > now());
        let pending = pending
            .ok_or_else(|| failed(format!("no comparison with {} is waiting; share again", peer.name)))?;
        self.update(|s| s.pairing = None)?;
        Ok(self.start_job(JobKind::Account, &format!("share {}", peer.id), move |d, _| {
            let (api, secrets) = d.account_api(true)?;
            if pending.code.unwrap_or_default().replace(' ', "") != digits {
                post(&api, &peer.id, &Mail::body("abort", &pending.pair, &[], 0))?;
                return Err(invalid(
                    "the codes differ: the key was not shared; someone may stand between the devices",
                ));
            }
            let (user, encrypted, saved) = {
                let s = d.saved();
                (s.user(), s.encrypted.unwrap_or(true), s.epoch)
            };
            let (epoch, bound) =
                (secrets.epoch.unwrap_or(saved), B64.decode(&pending.transcript).unwrap_or_default());
            let (to, theirs) = (aad(user, &peer.id, epoch, encrypted, &bound), B64.decode(&pending.key));
            let theirs = theirs.map_err(|_| failed("the comparison is broken; share again"))?;
            let sealed = seal_key(&secrets.device_key()?, &theirs, &to, &secrets.sync_key()?);
            post(&api, &peer.id, &Mail::body("key", &pending.pair, &sealed.map_err(sync_error)?, epoch))?;
            d.pin(&peer.id, &pending.key)?;
            Ok(json!({"device": peer.name}))
        }))
    }
}
