use crate::said;
use crate::state::{api, linked, read, update};
use crate::{Error, keys, vault};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use reqwest::Method;
use serde_json::{Value, json};
use wprs_sync::{Api, Lock, MIN_PASSPHRASE, STRONG, Secrets, encode, escrow_epoch, unseal};
use zeroize::Zeroizing;

const PATH: &str = "/api/v1/sync/escrow";

async fn stored(api: &Api) -> Result<Option<Vec<u8>>, Error> {
    let found = api.get(PATH).await?;
    Ok(found["escrow"].as_str().and_then(|text| B64.decode(text).ok()))
}

async fn put(api: &Api, escrow: Option<Vec<u8>>) -> Result<(), Error> {
    let body = json!({"escrow": escrow.map(|bytes| B64.encode(bytes))});
    Ok(api.call(Method::PUT, PATH, Some(body)).await.map(drop)?)
}

pub(crate) async fn set(passphrase: Zeroizing<String>) -> Result<Value, Error> {
    linked()?;
    if !read(|s| s.ready) {
        return Err(said(
            "android.phone_sync_key",
            "this phone has no sync key yet; join from a linked device",
            &[],
        ));
    }
    if read(|s| s.encrypted) != Some(true) {
        return Err(said(
            "android.phrase_encryption",
            "a recovery passphrase needs end-to-end encryption",
            &[],
        ));
    }
    if !passphrase.is_empty() && passphrase.trim().chars().count() < MIN_PASSPHRASE {
        return Err(said(
            "android.phrase_short",
            "a recovery passphrase has at least %1 characters",
            &[&MIN_PASSPHRASE],
        ));
    }
    let (api, mut secrets) = api()?;
    let (user, epoch) = read(|s| (s.user(), s.epoch));
    let epoch = secrets.epoch.unwrap_or(epoch);
    let stale = read(|s| s.recovery.is_some_and(|kept| kept != epoch));
    let old = if stale && !passphrase.is_empty() { stored(&api).await? } else { None };
    if old.is_some_and(|old| unseal(&old, &passphrase, user).is_ok()) {
        return Err(said(
            "android.phrase_same",
            "use another passphrase: the one before may be known to a revoked device",
            &[],
        ));
    }
    let lock = match passphrase.is_empty() {
        true => None,
        false => Some(Lock::new(&passphrase, STRONG)?),
    };
    let sealed = lock.as_ref().map(|l| l.seal(&secrets.sync_key()?, user, epoch)).transpose()?;
    put(&api, sealed).await?;
    secrets.lock = lock.map_or_else(String::new, |l| encode(&l.bytes()));
    vault::store(&secrets)?;
    update(|s| (s.recovery, s.recovery_stale) = ((!passphrase.is_empty()).then_some(epoch), false))?;
    Ok(json!({"recovery": if passphrase.is_empty() { "off" } else { "current" }}))
}

pub(crate) async fn restore(passphrase: Zeroizing<String>) -> Result<Value, Error> {
    linked()?;
    if read(|s| s.ready) {
        return Err(said("android.phone_holds_key", "this phone already holds the sync key", &[]));
    }
    let (api, mut secrets) = api()?;
    let state = api.get("/api/v1/sync/state").await?;
    let none = || said("android.no_escrow", "the account has no recovery escrow", &[]);
    let escrow = stored(&api).await?.ok_or_else(none)?;
    let (key, epoch, lock) = unseal(&escrow, &passphrase, read(|s| s.user()))?;
    let seen = read(|s| s.epoch);
    if epoch < seen {
        return Err(said(
            "android.escrow_replayed",
            "the recovery escrow holds key epoch %1, older than epoch %2 that this phone already saw: the server may replay an old escrow; join from a device",
            &[&epoch, &seen],
        ));
    }
    let current =
        state["epoch"].as_i64() == Some(epoch) && state["key_id"].as_str().is_none_or(|id| id == key.id());
    if !current {
        return Err(said(
            "android.escrow_old_key",
            "the recovery escrow holds an older key of the account; join from a device",
            &[],
        ));
    }
    secrets.set_key(&key, epoch);
    secrets.lock = encode(&lock.bytes());
    vault::store(&secrets)?;
    update(|s| {
        (s.ready, s.stale, s.epoch, s.recovery, s.recovery_stale) = (true, false, epoch, Some(epoch), false);
        (s.replica.seq, s.quarantine, s.unapplied, s.last_error, s.last_reply) = Default::default();
        s.pinned.get_or_insert_default();
    })?;
    keys::rekeyed(&api).await?;
    Ok(json!({"epoch": epoch}))
}

pub(crate) async fn keep(api: &Api, secrets: &mut Secrets, state: &Value) -> Result<(), Error> {
    let escrow = match state["escrow"] == true {
        true => stored(api).await.ok().flatten(),
        false => None,
    };
    let other = escrow.as_ref().is_some_and(|e| secrets.escrow_lock().is_some_and(|lock| !lock.fits(e)));
    if other {
        secrets.lock.clear();
        vault::store(secrets)?;
    }
    let sealed = escrow.as_deref().and_then(escrow_epoch);
    let stale = sealed.is_some_and(|epoch| Some(epoch) < state["epoch"].as_i64());
    update(|s| (s.recovery, s.recovery_stale) = (sealed, stale))
}

pub(crate) async fn reseal(api: &Api, secrets: &Secrets) -> Result<(), Error> {
    let Some(lock) = secrets.escrow_lock() else { return Ok(()) };
    if !stored(api).await?.is_some_and(|escrow| lock.fits(&escrow)) {
        return Ok(());
    }
    let (user, saved) = read(|s| (s.user(), s.epoch));
    let epoch = secrets.epoch.unwrap_or(saved);
    put(api, Some(lock.seal(&secrets.sync_key()?, user, epoch)?)).await?;
    update(|s| (s.recovery, s.recovery_stale) = (Some(epoch), false))
}
