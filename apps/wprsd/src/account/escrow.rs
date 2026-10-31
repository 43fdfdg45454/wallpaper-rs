use super::secrets::{Secrets, encode};
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use reqwest::Method;
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_core::model::JobKind;
use wprs_sync::{Api, Lock, MIN_PASSPHRASE, STRONG, escrow_epoch, unseal};
use zeroize::Zeroizing;

const PATH: &str = "/api/v1/sync/escrow";

fn stored(api: &Api) -> Result<Option<Vec<u8>>> {
    let found = block_on(api.get(PATH)).map_err(sync_error)?;
    Ok(found["escrow"].as_str().and_then(|text| B64.decode(text).ok()))
}

fn put(api: &Api, escrow: Option<Vec<u8>>) -> Result<()> {
    let body = json!({"escrow": escrow.map(|bytes| B64.encode(bytes))});
    block_on(api.call(Method::PUT, PATH, Some(body))).map(drop).map_err(sync_error)
}

impl Daemon {
    fn mark(&self) -> (i64, i64) {
        let s = self.saved();
        (s.user(), s.epoch)
    }

    pub fn set_escrow(self: &Arc<Self>, passphrase: Zeroizing<String>) -> Result<u64> {
        self.require_ready()?;
        if self.saved().encrypted != Some(true) {
            return Err(failed("a recovery passphrase needs end-to-end encryption"));
        }
        if !passphrase.is_empty() && passphrase.trim().chars().count() < MIN_PASSPHRASE {
            return Err(invalid(format!("a recovery passphrase has at least {MIN_PASSPHRASE} characters")));
        }
        Ok(self.start_job(JobKind::Account, "escrow", move |d, _| {
            let (api, mut secrets) = d.account_api(true)?;
            let (user, saved) = d.mark();
            let epoch = secrets.epoch.unwrap_or(saved);
            let stale = d.saved().recovery.is_some_and(|kept| kept != epoch);
            let old = if stale && !passphrase.is_empty() { stored(&api)? } else { None };
            if old.is_some_and(|old| unseal(&old, &passphrase, user).is_ok()) {
                return Err(invalid(crate::words::same_passphrase()));
            }
            let lock = match passphrase.is_empty() {
                true => None,
                false => Some(Lock::new(&passphrase, STRONG).map_err(sync_error)?),
            };
            let sealed = lock.as_ref().map(|l| l.seal(&secrets.sync_key()?, user, epoch).map_err(sync_error));
            put(&api, sealed.transpose()?)?;
            secrets.lock = lock.map_or_else(String::new, |l| encode(&l.bytes()));
            d.store_secrets(&secrets)?;
            d.update(|s| s.recovery = (!passphrase.is_empty()).then_some(epoch))?;
            tracing::info!(set = !passphrase.is_empty(), "the recovery escrow changed");
            Ok(json!({"recovery": if passphrase.is_empty() { "off" } else { "current" }}))
        }))
    }

    pub fn restore(self: &Arc<Self>, passphrase: Zeroizing<String>) -> Result<u64> {
        self.linked()?;
        if self.saved().ready {
            return Err(invalid("this computer already holds the sync key"));
        }
        Ok(self.start_job(JobKind::Link, "restore", move |d, _| {
            let (api, mut secrets) = d.account_api(true)?;
            let state = block_on(api.get("/api/v1/sync/state")).map_err(sync_error)?;
            let escrow = stored(&api)?.ok_or_else(|| failed("the account has no recovery escrow"))?;
            let (key, epoch, lock) = unseal(&escrow, &passphrase, d.saved().user()).map_err(sync_error)?;
            let seen = d.saved().epoch;
            if epoch < seen {
                return Err(failed(format!(
                    "the recovery escrow holds key epoch {epoch}, older than epoch {seen} that this computer \
                     already saw: the server may replay an old escrow; join from a device"
                )));
            }
            let current = state["epoch"].as_i64() == Some(epoch)
                && state["key_id"].as_str().is_none_or(|id| id == key.id());
            if !current {
                return Err(failed(
                    "the recovery escrow holds an older key of the account; join from a device",
                ));
            }
            secrets.set_key(&key, epoch);
            secrets.lock = encode(&lock.bytes());
            d.store_secrets(&secrets)?;
            d.update(|s| {
                (s.ready, s.stale, s.epoch, s.recovery, s.last_error) =
                    (true, false, epoch, Some(epoch), None);
                (s.replica.seq, s.quarantine, s.unapplied) = Default::default();
                s.pinned.get_or_insert_default();
            })?;
            d.rekeyed(&api)?;
            d.account.wake.notify_one();
            d.account.restart.notify_one();
            tracing::info!("the sync key was restored from the recovery escrow");
            Ok(json!({"epoch": epoch}))
        }))
    }

    pub fn keep_escrow(&self, api: &Api, secrets: &mut Secrets, state: &Value) -> Result<()> {
        let Some(escrow) = state["escrow"].as_bool().filter(|set| *set).map(|_| stored(api)).transpose()?
        else {
            return self.update(|s| s.recovery = None);
        };
        let other = escrow.as_ref().is_some_and(|e| secrets.escrow_lock().is_some_and(|lock| !lock.fits(e)));
        if other {
            secrets.lock.clear();
            self.store_secrets(secrets)?;
        }
        self.update(|s| s.recovery = escrow.as_deref().and_then(escrow_epoch))
    }

    pub fn reseal(&self, api: &Api, secrets: &Secrets) -> Result<()> {
        let Some(lock) = secrets.escrow_lock() else { return Ok(()) };
        if !stored(api)?.is_some_and(|escrow| lock.fits(&escrow)) {
            return Ok(());
        }
        let (user, saved) = self.mark();
        let epoch = secrets.epoch.unwrap_or(saved);
        put(api, Some(lock.seal(&secrets.sync_key()?, user, epoch).map_err(sync_error)?))?;
        self.update(|s| s.recovery = Some(epoch))
    }
}
