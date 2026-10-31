use crate::keys::{pins, rekeyed};
use crate::said;
use crate::state::{read, update};
use crate::{Error, vault};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use wprs_sync::{Api, Mail, Secrets, Session, aad, ack, encode, inbox, open_key, post, rekey, seal_key};
use zeroize::Zeroizing;

fn marks() -> (String, i64, bool, i64) {
    read(|s| (s.me(), s.user(), s.encrypted.unwrap_or(true), s.epoch))
}

pub(crate) async fn current(api: &Api, secrets: &mut Secrets) -> Result<Value, Error> {
    let (mut state, encrypted) =
        (api.get("/api/v1/sync/state").await?, read(|s| s.encrypted.unwrap_or(true)));
    if state["encrypted"].as_bool() != Some(encrypted) {
        return Err(said(
            "android.server_changed_encryption",
            "the server changed the encryption of this account; nothing was synced",
            &[],
        ));
    }
    if secrets.key.is_empty() {
        return Err(said(
            "android.phone_sync_key",
            "this phone has no sync key yet; join from a linked device",
            &[],
        ));
    }
    if encrypted && !secrets.next.is_empty() {
        rotate(api, secrets).await?;
        state = api.get("/api/v1/sync/state").await?;
    }
    let (server, id) = (state["epoch"].as_i64().unwrap_or_default(), state["key_id"].as_str());
    let (mut epoch, mut adopted) = (secrets.epoch.unwrap_or(marks().3), None);
    while encrypted && server > epoch {
        let Some(found) = adopt(api, secrets, epoch + 1, id.filter(|_| epoch + 1 == server)).await? else {
            break;
        };
        (epoch, adopted) = (epoch + 1, Some(found));
    }
    let key = secrets.sync_key()?;
    if server != epoch || id.is_some_and(|id| id != key.id()) {
        update(|s| (s.ready, s.stale) = (false, true))?;
        return Err(said(
            "android.sync_key_account",
            "the sync key of the account changed: waiting for the new key, or join again",
            &[],
        ));
    }
    if let Some((from, bound)) = adopted {
        if rekeyed(api).await? {
            secrets.lock.clear();
            vault::store(secrets)?;
        }
        let others = pins()?.into_keys().filter(|id| *id != from).collect();
        hand_over(api, secrets, epoch, &*bound, Some(others)).await?;
    }
    if id.is_none() {
        api.post("/api/v1/sync/epoch", json!({"epoch": server, "key_id": key.id()})).await?;
    }
    update(|s| (s.ready, s.stale, s.epoch) = (true, false, server))?;
    resume(api, secrets).await?;
    Ok(state)
}

async fn adopt(
    api: &Api,
    secrets: &mut Secrets,
    epoch: i64,
    key_id: Option<&str>,
) -> Result<Option<(String, Zeroizing<[u8; 32]>)>, Error> {
    let (old, mine, pins) = (secrets.sync_key()?, secrets.device_key()?, pins()?);
    let (me, user, encrypted, _) = marks();
    let check = old.check(epoch);
    let to = aad(user, &me, epoch, encrypted, &*check);
    for letter in inbox(api).await?.into_iter().filter(|l| l.rotation()) {
        let at = letter.mail.as_ref().map_or(i64::MIN, |m| m.epoch);
        let theirs = pins.get(&letter.from).and_then(|k| B64.decode(k).ok());
        let (Some(theirs), Ok(mail)) = (theirs.filter(|_| at == epoch), &letter.mail) else {
            if at < epoch {
                ack(api, letter.id).await?;
            }
            continue;
        };
        let opened = mail.bytes().ok().and_then(|sealed| open_key(&mine, &theirs, &to, &sealed).ok());
        ack(api, letter.id).await?;
        if let Some(key) = opened.filter(|k| key_id.is_none_or(|id| id == k.id())) {
            secrets.set_key(&key, epoch);
            vault::store(secrets)?;
            return Ok(Some((letter.from, check)));
        }
    }
    Ok(None)
}

pub(crate) async fn rotate(api: &Api, secrets: &mut Secrets) -> Result<Option<Value>, Error> {
    let (old, epoch) = (secrets.sync_key()?, secrets.epoch.unwrap_or(marks().3));
    let next = match secrets.next_key()? {
        Some(next) => next,
        None => {
            secrets.next = encode(&old.rotated().bytes());
            vault::store(secrets)?;
            secrets
                .next_key()?
                .ok_or_else(|| said("android.new_key_could", "the new key could not be stored", &[]))?
        }
    };
    match api.post("/api/v1/sync/epoch", json!({"epoch": epoch + 1, "key_id": next.id()})).await {
        Err(e) if e.code() == "epoch_mismatch" => {
            secrets.next.clear();
            vault::store(secrets)?;
            return Ok(None);
        }
        claimed => drop(claimed?),
    }
    let (_, user, encrypted, _) = marks();
    let bound = old.check(epoch + 1);
    let mut session = Session::new(api.clone(), user, epoch + 1, encrypted, next);
    session.old = Some(old);
    let records = rekey(&session, epoch + 1).await?;
    api.post("/api/v1/sync/epoch", json!({"epoch": epoch + 1, "done": true})).await?;
    secrets.set_key(&session.key, epoch + 1);
    secrets.lock.clear();
    vault::store(secrets)?;
    update(|s| (s.epoch, s.ready, s.stale) = (epoch + 1, true, false))?;
    rekeyed(api).await?;
    let shared = hand_over(api, secrets, epoch + 1, &*bound, None).await?;
    Ok(Some(json!({"epoch": epoch + 1, "records": records, "shared": shared})))
}

async fn hand_over(
    api: &Api,
    secrets: &mut Secrets,
    epoch: i64,
    bound: &[u8],
    targets: Option<BTreeSet<String>>,
) -> Result<Vec<String>, Error> {
    let (me, user, encrypted, _) = marks();
    let (mine, key, peers) = (secrets.device_key()?, secrets.sync_key()?, read(|s| s.peers.clone()));
    let wanted = |id: &String| *id != me && targets.as_ref().is_none_or(|t| t.contains(id));
    let (mut shared, mut waiting) = (Vec::new(), BTreeSet::new());
    for (id, theirs) in pins()?.iter().filter(|(id, _)| wanted(id)) {
        let to = aad(user, id, epoch, encrypted, bound);
        let theirs = B64
            .decode(theirs)
            .map_err(|_| said("android.pinned_device_key", "a pinned device key is broken", &[]));
        let sealed = theirs.and_then(|theirs| Ok(seal_key(&mine, &theirs, &to, &key)?));
        let sent = match sealed {
            Ok(sealed) => post(api, id, &Mail::body("rotate", "", &sealed, epoch)).await.map_err(Error::from),
            Err(e) => Err(e),
        };
        match sent {
            Ok(()) => shared.push(peers.iter().find(|p| p.id == *id).map_or(id.clone(), |p| p.name.clone())),
            Err(e) => {
                log::warn!("the new key waits to reach a device: {e}");
                waiting.insert(id.clone());
            }
        }
    }
    secrets.handover = (!waiting.is_empty()).then(|| (epoch, encode(bound), waiting));
    vault::store(secrets)?;
    Ok(shared)
}

async fn resume(api: &Api, secrets: &mut Secrets) -> Result<(), Error> {
    let Some((epoch, bound, waiting)) = secrets.handover.clone() else { return Ok(()) };
    let bound = Zeroizing::new(B64.decode(&bound).unwrap_or_default());
    match epoch == secrets.epoch.unwrap_or(marks().3) {
        true => hand_over(api, secrets, epoch, &bound, Some(waiting)).await.map(drop),
        false => {
            secrets.handover = None;
            vault::store(secrets)
        }
    }
}
