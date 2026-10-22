use super::saved::Peer;
use super::server::pause;
use super::{block_on, sync_error};
use crate::error::{Result, failed};
use crate::jobs::Ctl;
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use reqwest::Method;
use serde_json::json;
use std::time::{Duration, Instant};
use wprs_sync::{Api, MAIL, Mail};

pub struct Letter {
    pub id: i64,
    pub from: String,
    pub mail: wprs_sync::Result<Mail>,
}

impl Letter {
    pub fn rotation(&self) -> bool {
        self.mail.as_ref().is_ok_and(|mail| mail.step == "rotate")
    }
}

pub fn inbox(api: &Api) -> Result<Vec<Letter>> {
    let found = block_on(api.fetch("/api/v1/mailbox", MAIL)).map_err(sync_error)?;
    let items = found["items"].as_array().into_iter().flatten().filter(|m| m["kind"] == "key-share");
    let letter = |m: &serde_json::Value| {
        let body = B64.decode(m["body"].as_str().unwrap_or_default()).unwrap_or_default();
        let (id, from) =
            (m["id"].as_i64().unwrap_or_default(), m["from_device"].as_str().unwrap_or_default());
        Letter { id, from: from.to_owned(), mail: Mail::read(&body) }
    };
    Ok(items.map(letter).collect())
}

pub fn ack(api: &Api, id: i64) -> Result<()> {
    block_on(api.call(Method::DELETE, &format!("/api/v1/mailbox/{id}"), None)).map(drop).map_err(sync_error)
}

pub fn post(api: &Api, to: &str, body: &[u8]) -> Result<()> {
    let body = json!({"to_device": to, "kind": "key-share", "body": B64.encode(body)});
    block_on(api.post("/api/v1/mailbox", body)).map(drop).map_err(sync_error)
}

pub fn from(api: &Api, peer: &str) -> Result<Vec<Letter>> {
    let found: Vec<Letter> = inbox(api)?.into_iter().filter(|l| l.from == peer && !l.rotation()).collect();
    found.iter().try_for_each(|l| ack(api, l.id))?;
    Ok(found)
}

pub fn next(api: &Api, peer: &Peer, pair: &str, step: &str) -> Result<Option<Mail>> {
    let Some(letter) = from(api, &peer.id)?.into_iter().next() else { return Ok(None) };
    let mail = letter.mail.map_err(sync_error)?;
    match mail.step.as_str() {
        _ if mail.pair != pair => Err(failed(format!(
            "{} sent a message of another comparison, replayed or out of order: start again",
            peer.name
        ))),
        "abort" => Err(failed(format!("{} stopped the comparison: the key was not shared", peer.name))),
        found if found == step => Ok(Some(mail)),
        found => Err(failed(format!("{} sent {found} instead of {step}: start again", peer.name))),
    }
}

pub fn wait(api: &Api, peer: &Peer, pair: &str, step: &str, ctl: &Ctl) -> Result<Mail> {
    let deadline = Instant::now() + Duration::from_secs(600);
    while Instant::now() < deadline {
        if let Some(mail) = next(api, peer, pair, step)? {
            return Ok(mail);
        }
        pause(ctl, Duration::from_secs(2))?;
    }
    Err(failed(format!("{} did not answer within 10 minutes; start again", peer.name)))
}
