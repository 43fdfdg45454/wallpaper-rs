use super::mail::{self, post};
use super::secrets::{Secrets, encode};
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Arc;
use wprs_core::model::JobKind;
use wprs_sync::{Api, Mail, Session, SyncKey, aad, open_key, rekey, seal_key};
use zeroize::Zeroizing;

impl Daemon {
    pub fn rotate_now(self: &Arc<Self>) -> Result<u64> {
        self.require_ready()?;
        if self.saved().encrypted == Some(false) {
            return Err(failed("sync is not encrypted: a new key would protect nothing"));
        }
        Ok(self.start_job(JobKind::Account, "rotate", |d, _| {
            let (api, mut secrets) = d.account_api(true)?;
            d.rotate(&api, &mut secrets, false)?
                .ok_or_else(|| failed("another device replaced the key first"))
        }))
    }

    pub fn rotate(&self, api: &Api, secrets: &mut Secrets, revoked: bool) -> Result<Option<Value>> {
        let (old, epoch) = (secrets.sync_key()?, secrets.epoch.unwrap_or(self.saved().epoch));
        let next = match secrets.next_key()? {
            Some(next) => next,
            None => {
                secrets.next = encode(&old.rotated().bytes());
                self.store_secrets(secrets)?;
                secrets.next_key()?.ok_or_else(|| failed("the new key could not be stored"))?
            }
        };
        let claimed =
            block_on(api.post("/api/v1/sync/epoch", json!({"epoch": epoch + 1, "key_id": next.id()})));
        match claimed {
            Err(e) if e.code() == "epoch_mismatch" => {
                secrets.next.clear();
                self.store_secrets(secrets)?;
                return Ok(None);
            }
            claimed => claimed.map_err(sync_error)?,
        };
        let (user, encrypted) = {
            let s = self.saved();
            (s.user(), s.encrypted.unwrap_or(true))
        };
        let bound = old.check(epoch + 1);
        let mut session = Session::new(api.clone(), user, epoch + 1, encrypted, next);
        session.old = Some(old);
        let records = block_on(rekey(&session, epoch + 1)).map_err(sync_error)?;
        block_on(api.post("/api/v1/sync/epoch", json!({"epoch": epoch + 1, "done": true})))
            .map_err(sync_error)?;
        secrets.set_key(&session.key, epoch + 1);
        self.store_secrets(secrets)?;
        self.update(|s| (s.epoch, s.ready, s.stale) = (epoch + 1, true, false))?;
        match self.rekeyed(api)? || revoked {
            true => secrets.lock.clear(),
            false => self.reseal(api, secrets)?,
        }
        let shared = self.hand_over(api, secrets, epoch + 1, &*bound, None)?;
        tracing::info!(epoch = epoch + 1, records, shared = shared.len(), "the sync key was replaced");
        Ok(Some(json!({"epoch": epoch + 1, "records": records, "shared": shared})))
    }

    pub fn hand_over(
        &self,
        api: &Api,
        secrets: &mut Secrets,
        epoch: i64,
        bound: &[u8],
        targets: Option<BTreeSet<String>>,
    ) -> Result<Vec<String>> {
        let pins = self.pins()?;
        let (me, user, encrypted, peers) = {
            let s = self.saved();
            (s.device.clone().unwrap_or_default(), s.user(), s.encrypted.unwrap_or(true), s.peers.clone())
        };
        let (mine, key) = (secrets.device_key()?, secrets.sync_key()?);
        let wanted = |id: &String| *id != me && targets.as_ref().is_none_or(|t| t.contains(id));
        let (mut shared, mut waiting) = (Vec::new(), BTreeSet::new());
        for (id, theirs) in pins.iter().filter(|(id, _)| wanted(id)) {
            let to = aad(user, id, epoch, encrypted, bound);
            let theirs = B64.decode(theirs).map_err(|_| failed("a pinned device key is broken"));
            let sealed = theirs.and_then(|theirs| seal_key(&mine, &theirs, &to, &key).map_err(sync_error));
            match sealed.and_then(|sealed| post(api, id, &Mail::body("rotate", "", &sealed, epoch))) {
                Ok(()) => {
                    shared.push(peers.iter().find(|p| p.id == *id).map_or(id.clone(), |p| p.name.clone()))
                }
                Err(e) => {
                    tracing::warn!("the new key waits to reach a device: {e}");
                    waiting.insert(id.clone());
                }
            }
        }
        secrets.handover = (!waiting.is_empty()).then(|| (epoch, encode(bound), waiting));
        self.store_secrets(secrets)?;
        Ok(shared)
    }

    pub fn resume_hand_over(&self, api: &Api, secrets: &mut Secrets) -> Result<()> {
        let Some((epoch, bound, waiting)) = secrets.handover.clone() else { return Ok(()) };
        let bound = Zeroizing::new(B64.decode(&bound).unwrap_or_default());
        let current = secrets.epoch.unwrap_or(self.saved().epoch);
        match epoch == current {
            true => self.hand_over(api, secrets, epoch, &bound, Some(waiting)).map(drop),
            false => {
                secrets.handover = None;
                self.store_secrets(secrets)
            }
        }
    }

    pub fn adopt(
        &self,
        api: &Api,
        secrets: &mut Secrets,
        epoch: i64,
        key_id: Option<&str>,
    ) -> Result<Option<(String, Zeroizing<[u8; 32]>)>> {
        let (old, mine, pins) = (secrets.sync_key()?, secrets.device_key()?, self.pins()?);
        let (me, user, encrypted) = {
            let s = self.saved();
            (s.device.clone().unwrap_or_default(), s.user(), s.encrypted.unwrap_or(true))
        };
        let check = old.check(epoch);
        let to = aad(user, &me, epoch, encrypted, &*check);
        for letter in mail::inbox(api)?.into_iter().filter(|l| l.rotation()) {
            let at = letter.mail.as_ref().map_or(i64::MIN, |m| m.epoch);
            let theirs = pins.get(&letter.from).and_then(|k| B64.decode(k).ok());
            let (Some(theirs), Ok(mail)) = (theirs.filter(|_| at == epoch), &letter.mail) else {
                if at < epoch {
                    mail::ack(api, letter.id)?;
                }
                continue;
            };
            let opened = mail.bytes().ok().and_then(|sealed| open_key(&mine, &theirs, &to, &sealed).ok());
            mail::ack(api, letter.id)?;
            if let Some(key) = opened.filter(|k| key_id.is_none_or(|id| id == k.id())) {
                secrets.set_key(&key, epoch);
                self.store_secrets(secrets)?;
                return Ok(Some((letter.from, check)));
            }
        }
        Ok(None)
    }

    pub fn reset_sync(self: &Arc<Self>) -> Result<u64> {
        self.linked()?;
        Ok(self.start_job(JobKind::Account, "reset", |d, _| {
            let (api, mut secrets) = d.account_api(true)?;
            let encrypted = {
                let s = d.saved();
                !(s.plaintext && s.encrypted == Some(false))
            };
            block_on(api.post("/api/v1/sync/reset", json!({"confirm": "reset"}))).map_err(sync_error)?;
            let key = secrets.sync_key().map_or_else(|_| SyncKey::generate(), |old| old.rotated());
            (secrets.records, secrets.held, secrets.lock, secrets.handover) = Default::default();
            let body = json!({"encrypted": encrypted, "confirm_plaintext": !encrypted, "key_id": key.id()});
            let state = block_on(api.post("/api/v1/sync/setup", body)).map_err(sync_error)?;
            if state["encrypted"].as_bool() != Some(encrypted) {
                return Err(failed("the server set up sync with another encryption than asked"));
            }
            let epoch = state["epoch"].as_i64().unwrap_or(0);
            secrets.set_key(&key, epoch);
            d.store_secrets(&secrets)?;
            d.update(|s| {
                (s.replica, s.present, s.quarantine) = Default::default();
                (s.legacy, s.pairing, s.pinned) = (Default::default(), None, Some(Default::default()));
                (s.kept, s.unapplied, s.offers, s.recovery) = Default::default();
                (s.files.stored, s.files.dropped, s.files.uploaded) = Default::default();
                (s.ready, s.stale, s.epoch, s.last_error, s.encrypted) =
                    (true, false, epoch, None, Some(encrypted));
            })?;
            d.rekeyed(&api)?;
            d.cycle(true)
        }))
    }
}
