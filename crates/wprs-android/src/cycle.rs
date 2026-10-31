use crate::link::PLATFORM;
use crate::state::{api, failed, linked, read, update};
use crate::{Error, apply, keys, local, rotation, vault};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use wprs_sync::{Session, View, arrived, carry, encode, hold, now_ms, rules};

pub(crate) async fn sync(conflict: &str) -> Result<Value, Error> {
    linked()?;
    if !read(|s| s.ready || s.stale) {
        return Err(failed("this phone has no sync key yet; join from a linked device"));
    }
    let result = cycle(conflict).await;
    let error = result.as_ref().err().map(ToString::to_string);
    update(|s| {
        s.last_sync = if error.is_none() { Some(crate::library::now()) } else { s.last_sync };
        s.last_error = error;
    })?;
    result
}

async fn cycle(conflict: &str) -> Result<Value, Error> {
    let (api, mut secrets) = api()?;
    let state = rotation::current(&api, &mut secrets).await?;
    let outdated = state["format"].as_i64() > Some(wprs_sync::FORMAT);
    let escrow = match state["escrow"] == true {
        true => api.get("/api/v1/sync/escrow").await.ok(),
        false => None,
    };
    let sealed =
        escrow.and_then(|e| B64.decode(e["escrow"].as_str()?).ok()).and_then(|b| wprs_sync::escrow_epoch(&b));
    let stale = sealed.is_some_and(|epoch| Some(epoch) < state["epoch"].as_i64());
    update(|s| (s.outdated, s.recovery_stale) = (outdated, stale))?;
    if outdated {
        return Err(failed("this account is synced by a newer version of wallpaper-rs: update the app"));
    }
    let saved = read(Clone::clone);
    let (device, encrypted, epoch) = (saved.me(), saved.encrypted.unwrap_or(true), saved.epoch);
    let before: BTreeSet<String> = match saved.members.is_empty() {
        true => saved.peers.iter().map(|p| p.id.clone()).collect(),
        false => saved.members.clone(),
    };
    let key = secrets.sync_key()?;
    let ids = key.ids();
    let session = Session::new(api.clone(), saved.user(), epoch, encrypted, key);
    let names = ids.named("names", "");
    let (mut replica, mut links, mut clock, mut kept) = match saved.names.is_empty() || saved.names == names {
        true => (saved.replica, saved.links, saved.clock, saved.kept),
        false => (Default::default(), Default::default(), saved.clock, Default::default()),
    };
    let present = if saved.names == names { saved.present } else { Default::default() };
    secrets.join(&mut replica);
    wprs_sync::removals(&ids, &saved.removed, &mut replica, now_ms());
    let mut local = local::snapshot(&ids, &replica, &mut links, &present, &mut kept)?;
    if encrypted {
        let fields = [
            ("name", json!(saved.label)),
            ("platform", json!(PLATFORM)),
            ("key", json!(encode(&secrets.device_key()?.public()))),
        ];
        let (key, mut own): (_, View) =
            (format!("device/{device}"), fields.map(|(k, v)| (k.to_owned(), v)).into());
        carry(&key, &mut own, &replica, &BTreeSet::new());
        local.views.insert(key, own);
    }
    let now = clock.tick(now_ms(), &device);
    let start = replica.bases.clone();
    let synced = wprs_sync::sync(&session, &mut replica, &local.views, rules, &now).await;
    if synced.as_ref().is_err_and(|e| e.code() == wprs_sync::TOO_OLD) {
        update(|s| s.outdated = true)?;
    }
    let synced = synced?;
    let (mut applied, unsent) = (synced.applied, synced.left);
    applied.extend(arrived(&local, &replica, &applied, &start));
    let (changed, errors) = local::apply(&local, applied, &mut links, &replica, &mut kept);
    let copied = apply::copies(&local, synced.copies, &mut links, &ids, conflict);
    let unapplied = hold(&mut replica, &start, &local, &errors, &now);
    if secrets.split(&mut replica) {
        vault::store(&secrets)?;
    }
    replica.bases.values().for_each(|(_, doc)| clock.observe(&doc.clock()));
    let (rejected, records) = (session.rejected(), replica.bases.len());
    let left =
        keys::refresh(&api).await.is_ok_and(|now| before.iter().any(|id| !now.iter().any(|p| &p.id == id)));
    keys::pins()?;
    let mut present: BTreeSet<String> = local.items.into_keys().collect();
    present.retain(|key| !unapplied.contains_key(&format!("library/{key}")));
    let (mut fresh, mut broken) = (0, 0);
    update(|s| {
        fresh = rejected.iter().filter(|(k, seq)| s.quarantine.get(*k) != Some(seq)).count();
        broken = unapplied.iter().filter(|(k, seq)| s.unapplied.get(*k) != Some(seq)).count();
        s.quarantine.extend(rejected);
        let base = |k: &String| replica.bases.get(k).or_else(|| secrets.records.get(k));
        s.quarantine.retain(|k, seq| base(k).is_none_or(|(base, _)| base < seq));
        (s.replica, s.links, s.clock, s.kept, s.names) = (replica, links, clock, kept, names);
        (s.present, s.unapplied) = (present, unapplied);
        s.removed.retain(|key, _| !saved.removed.contains_key(key));
        s.warning = state["warning"].as_str().map(String::from);
    })?;
    if encrypted && (state["rotate"] == true || left) {
        rotation::rotate(&api, &mut secrets).await?;
    }
    if fresh > 0 {
        return Err(failed(format!(
            "{fresh} records cannot be decrypted with this sync key; they are skipped from now on"
        )));
    }
    if let Some((_, error)) = errors.first().filter(|_| broken > 0) {
        return Err(failed(format!(
            "{broken} synced changes cannot be applied on this phone and are tried again at every sync: {error}"
        )));
    }
    Ok(json!({"applied": changed + copied, "records": records, "unsent": unsent}))
}
