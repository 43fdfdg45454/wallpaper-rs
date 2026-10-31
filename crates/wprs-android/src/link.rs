use crate::said;
use crate::state::{self, pause, read, update};
use crate::{Error, keys, vault};
use reqwest::{Method, Url};
use serde_json::{Value, json};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use wprs_sync::{AGENT, Api, DeviceKey, Poll, Secrets, SyncKey, encode, poll};

pub(crate) use crate::server::add;

pub(crate) const PLATFORM: &str = "android";

struct Pending {
    api: Api,
    key: DeviceKey,
    code: String,
    interval: u64,
    until: Instant,
    label: String,
    plaintext: bool,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static SHOWN: Mutex<Option<(String, String)>> = Mutex::new(None);

fn same_origin(a: &str, b: &str) -> bool {
    let origin = |url: &str| Url::parse(url).ok().map(|u| u.origin());
    origin(a).is_some() && origin(a) == origin(b)
}

pub(crate) fn shown() -> Option<Value> {
    let shown = SHOWN.lock().unwrap_or_else(PoisonError::into_inner).clone();
    shown.map(|(code, uri)| json!({"code": code, "uri": uri}))
}

pub(crate) fn cancel() {
    PENDING.lock().unwrap_or_else(PoisonError::into_inner).take();
    SHOWN.lock().unwrap_or_else(PoisonError::into_inner).take();
}

pub(crate) async fn start(label: &str, plaintext: bool) -> Result<Value, Error> {
    let url = read(|s| s.url.clone());
    match () {
        _ if url.is_empty() => return Err(said("android.add_server", "add a server first", &[])),
        _ if read(|s| s.device.is_some()) => {
            return Err(said("android.phone_already_linked", "this phone is already linked", &[]));
        }
        _ => {}
    }
    let label = label.trim().to_owned();
    if label.is_empty() || label.chars().count() > 64 || label.chars().any(char::is_control) {
        return Err(said(
            "android.device_name_printable",
            "a device name has 1 to 64 printable characters",
            &[],
        ));
    }
    let (key, api) = (DeviceKey::generate(), Api::pinned(&url, read(|s| s.pin.clone()).as_deref())?);
    let body = json!({"platform": PLATFORM, "enc_key": encode(&key.public()), "client": AGENT});
    let start = api.post("/api/v1/auth/device/start", body).await?;
    let text = |field: &str| start[field].as_str().unwrap_or_default().to_owned();
    let (interval, expires) = (start["interval"].as_u64(), start["expires_in"].as_u64());
    let (interval, expires) = (interval.unwrap_or(5).clamp(1, 60), expires.unwrap_or(600).min(1800));
    let until = Instant::now() + Duration::from_secs(expires);
    let code = text("device_code");
    *PENDING.lock().unwrap_or_else(PoisonError::into_inner) =
        Some(Pending { api, key, code, interval, until, label, plaintext });
    let uri = Some(text("verification_uri")).filter(|uri| same_origin(uri, &url));
    let shown = (text("user_code"), uri.unwrap_or_else(|| format!("{url}/link")));
    *SHOWN.lock().unwrap_or_else(PoisonError::into_inner) = Some(shown.clone());
    Ok(json!({"code": shown.0, "uri": shown.1}))
}

pub(crate) async fn wait() -> Result<Value, Error> {
    let pending = PENDING.lock().unwrap_or_else(PoisonError::into_inner).take();
    let pending =
        pending.ok_or_else(|| said("android.link_waiting_link", "no link is waiting; link again", &[]))?;
    let done = approved(&pending).await;
    SHOWN.lock().unwrap_or_else(PoisonError::into_inner).take();
    finish(pending, done?).await
}

async fn approved(pending: &Pending) -> Result<Value, Error> {
    let mut interval = pending.interval;
    while Instant::now() < pending.until {
        pause(interval).await?;
        let answer = pending.api.post("/api/v1/auth/device/token", json!({"device_code": pending.code}));
        match poll(&mut interval, answer.await) {
            Poll::Done(token) => return Ok(token),
            Poll::Wait(_) => {}
            Poll::Stop(e) if e.code() == "access_denied" => {
                return Err(said("android.link_refused", "the link was refused", &[]));
            }
            Poll::Stop(e) => return Err(e.into()),
        }
    }
    Err(said("android.code_expired_before", "the code expired before it was approved; link again", &[]))
}

async fn finish(pending: Pending, token: Value) -> Result<Value, Error> {
    let text = |field: &str| token[field].as_str().unwrap_or_default().to_owned();
    let mut secrets = Secrets::default();
    (secrets.token, secrets.device) = (text("token"), encode(&pending.key.bytes()));
    vault::store(&secrets)?;
    let api = pending.api.with_token(&secrets.token);
    let me = api.get("/api/v1/me").await?;
    let existing = match api.get("/api/v1/sync/state").await {
        Err(e) if e.code() == "sync_not_set_up" => None,
        state => Some(state?),
    };
    if existing.as_ref().is_some_and(|state| state["encrypted"] != true) && !pending.plaintext {
        let path = format!("/api/v1/me/devices/{}", text("device_id"));
        drop(api.call(Method::DELETE, &path, None).await);
        vault::forget()?;
        return Err(said(
            "android.account_syncs_without",
            "the account syncs without end-to-end encryption; link with plain text only if you accept that",
            &[],
        ));
    }
    let owner = (read(|s| s.url.clone()), me["id"].as_i64().unwrap_or_default());
    update(|s| {
        if s.owner.as_ref() != Some(&owner) {
            (s.replica, s.links, s.clock, s.present, s.kept) = Default::default();
            (s.quarantine, s.unapplied, s.names) = Default::default();
            s.owner = Some(owner.clone());
        }
        (s.device, s.device_name, s.label) =
            (Some(text("device_id")), Some(text("device_name")), pending.label);
        (s.plaintext, s.pinned, s.members) =
            (pending.plaintext, Some(Default::default()), Default::default());
        s.handle = me["handle"].as_str().map(String::from);
    })?;
    let state = match existing {
        Some(state) => state,
        None => {
            let sync = SyncKey::generate();
            secrets.set_key(&sync, 0);
            vault::store(&secrets)?;
            let (encrypted, plaintext) = (!pending.plaintext, pending.plaintext);
            let body = json!({"encrypted": encrypted, "confirm_plaintext": plaintext, "key_id": sync.id()});
            let state = api.post("/api/v1/sync/setup", body).await?;
            if state["encrypted"].as_bool() != Some(encrypted) {
                return Err(said(
                    "android.server_set_up",
                    "the server set up sync with another encryption than asked",
                    &[],
                ));
            }
            secrets.set_key(&sync, state["epoch"].as_i64().unwrap_or(0));
            vault::store(&secrets)?;
            state
        }
    };
    update(|s| {
        (s.encrypted, s.epoch) = (state["encrypted"].as_bool(), state["epoch"].as_i64().unwrap_or(0));
        (s.warning, s.ready, s.stale) =
            (state["warning"].as_str().map(String::from), !secrets.key.is_empty(), false);
        (s.last_error, s.last_reply, s.last_sync) = (None, None, None);
    })?;
    drop(keys::refresh(&api).await);
    let needs_key = secrets.key.is_empty();
    Ok(
        json!({"device": text("device_name"), "encrypted": state["encrypted"], "needs_key": needs_key, "active": state::active()}),
    )
}
