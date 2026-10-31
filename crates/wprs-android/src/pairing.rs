use crate::keys::{peer, pin, rekeyed};
use crate::said;
use crate::state::{api, linked, pause, read, update};
use crate::{Error, rotation, vault};
use serde_json::{Value, json};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use wprs_sync::{
    Api, Mail, Peer, aad, code, commit, from, hex, next, nonce, open_key, post, random, seal_key, transcript,
};

const PATIENCE: Duration = Duration::from_secs(600);

struct Pairing {
    peer: Peer,
    theirs: Vec<u8>,
    pair: String,
    bound: [u8; 32],
    code: String,
    sharing: bool,
    epoch: i64,
    key_id: Option<String>,
}

static PAIRING: Mutex<Option<Pairing>> = Mutex::new(None);

pub(crate) fn shown() -> Option<Value> {
    let pairing = PAIRING.lock().unwrap_or_else(PoisonError::into_inner);
    pairing
        .as_ref()
        .map(|p| json!({"device": p.peer.id, "name": p.peer.name, "code": p.code, "sharing": p.sharing}))
}

pub(crate) fn cancel() {
    PAIRING.lock().unwrap_or_else(PoisonError::into_inner).take();
}

async fn wait(api: &Api, peer: &Peer, pair: &str, step: &str) -> Result<Mail, Error> {
    let until = Instant::now() + PATIENCE;
    while Instant::now() < until {
        if let Some(mail) = next(api, peer, pair, step).await? {
            return Ok(mail);
        }
        pause(2).await?;
    }
    Err(said("android.did_answer_within", "%1 did not answer within 10 minutes; start again", &[&peer.name]))
}

fn keep(pairing: Pairing) -> Value {
    let shown = json!({"code": pairing.code, "device": pairing.peer.name});
    *PAIRING.lock().unwrap_or_else(PoisonError::into_inner) = Some(pairing);
    shown
}

pub(crate) async fn join(device: &str) -> Result<Value, Error> {
    linked()?;
    if read(|s| s.ready) {
        return Err(said("android.phone_already_holds", "this phone already holds the sync key", &[]));
    }
    cancel();
    let (api, secrets) = api()?;
    let (mine, peer) = (secrets.device_key()?, peer(&api, device).await?);
    let (theirs, state) = (peer.public()?, api.get("/api/v1/sync/state").await?);
    from(&api, &peer.id).await?;
    let (pair, own) = (hex(&random::<16>()), random::<32>());
    post(&api, &peer.id, &Mail::body("commit", &pair, &commit(pair.as_bytes(), &own), 0)).await?;
    let other = nonce(&wait(&api, &peer, &pair, "nonce").await?)?;
    post(&api, &peer.id, &Mail::body("reveal", &pair, &own, 0)).await?;
    let bound = transcript(pair.as_bytes(), (&mine.public(), &own), (&theirs, &other));
    let (epoch, key_id) = (state["epoch"].as_i64().unwrap_or(0), state["key_id"].as_str().map(String::from));
    Ok(keep(Pairing { code: code(&bound), peer, theirs, pair, bound, sharing: false, epoch, key_id }))
}

pub(crate) async fn share(device: &str) -> Result<Value, Error> {
    linked()?;
    if !read(|s| s.ready) {
        return Err(said(
            "android.phone_sync_key",
            "this phone has no sync key yet; join from a linked device",
            &[],
        ));
    }
    cancel();
    let (api, mut secrets) = api()?;
    rotation::current(&api, &mut secrets).await?;
    let (mine, peer) = (secrets.device_key()?, peer(&api, device).await?);
    let (theirs, until) = (peer.public()?, Instant::now() + PATIENCE);
    let opened = loop {
        if let Some(letter) = from(&api, &peer.id).await?.pop() {
            break letter.mail?;
        }
        if Instant::now() > until {
            return Err(said(
                "android.did_ask_key",
                "%1 did not ask for the key within 10 minutes",
                &[&peer.name],
            ));
        }
        pause(2).await?;
    };
    if opened.step != "commit" || opened.pair.len() != 32 {
        return Err(said(
            "android.sent_instead_request",
            "%1 sent %2 instead of a request: start again",
            &[&peer.name, &opened.step],
        ));
    }
    let (pair, own) = (opened.pair.clone(), random::<32>());
    post(&api, &peer.id, &Mail::body("nonce", &pair, &own, 0)).await?;
    let other = nonce(&wait(&api, &peer, &pair, "reveal").await?)?;
    if opened.bytes()? != commit(pair.as_bytes(), &other) {
        post(&api, &peer.id, &Mail::body("abort", &pair, &[], 0)).await?;
        return Err(said(
            "android.joining_device_broke",
            "the joining device broke its commitment: someone stands between the devices",
            &[],
        ));
    }
    let bound = transcript(pair.as_bytes(), (&theirs, &other), (&mine.public(), &own));
    let epoch = secrets.epoch.unwrap_or(read(|s| s.epoch));
    Ok(keep(Pairing { code: code(&bound), peer, theirs, pair, bound, sharing: true, epoch, key_id: None }))
}

pub(crate) async fn confirm(typed: &str) -> Result<Value, Error> {
    let digits: String = typed.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 6 {
        return Err(said("android.code_digits", "the code has 6 digits", &[]));
    }
    let pairing = PAIRING.lock().unwrap_or_else(PoisonError::into_inner).take();
    let p = pairing.ok_or_else(|| {
        said("android.comparison_waiting_start", "no comparison is waiting; start again", &[])
    })?;
    let (api, mut secrets) = api()?;
    if p.code.replace(' ', "") != digits {
        post(&api, &p.peer.id, &Mail::body("abort", &p.pair, &[], 0)).await?;
        return Err(said(
            "android.codes_differ_key",
            "the codes differ: the key did not move; someone may stand between the devices",
            &[],
        ));
    }
    let (user, me, encrypted) = read(|s| (s.user(), s.me(), s.encrypted.unwrap_or(true)));
    let key = p.peer.enc_key.clone().unwrap_or_default();
    if p.sharing {
        let to = aad(user, &p.peer.id, p.epoch, encrypted, &p.bound);
        let sealed = seal_key(&secrets.device_key()?, &p.theirs, &to, &secrets.sync_key()?)?;
        post(&api, &p.peer.id, &Mail::body("key", &p.pair, &sealed, p.epoch)).await?;
        pin(&p.peer.id, &key)?;
        return Ok(json!({"device": p.peer.name}));
    }
    let until = Instant::now() + PATIENCE;
    let sealed = loop {
        if let Some(mail) = next(&api, &p.peer, &p.pair, "key").await? {
            break mail.bytes()?;
        }
        if Instant::now() > until {
            return Err(said(
                "android.did_send_key",
                "%1 did not send the key within 10 minutes; start again",
                &[&p.peer.name],
            ));
        }
        pause(1).await?;
    };
    let to = aad(user, &me, p.epoch, encrypted, &p.bound);
    let opened = open_key(&secrets.device_key()?, &p.theirs, &to, &sealed).map_err(|_| {
        said(
            "android.key_does_open",
            "the key from %1 does not open here: the server changed keys or settings",
            &[&p.peer.name],
        )
    })?;
    if p.key_id.as_ref().is_some_and(|id| *id != opened.id()) {
        return Err(said(
            "android.sent_key_current",
            "%1 sent a key that is not the current key of the account",
            &[&p.peer.name],
        ));
    }
    secrets.set_key(&opened, p.epoch);
    vault::store(&secrets)?;
    pin(&p.peer.id, &key)?;
    rekeyed(&api).await?;
    update(|s| {
        (s.ready, s.stale, s.last_error, s.last_reply, s.epoch) = (true, false, None, None, p.epoch);
        (s.replica.seq, s.quarantine, s.unapplied) = Default::default();
    })?;
    Ok(json!({"from": p.peer.name}))
}
