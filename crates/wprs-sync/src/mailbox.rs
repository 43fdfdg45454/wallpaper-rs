use crate::{Api, Error, MAIL, Mail, Result};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub enc_key: Option<String>,
    pub created_at: i64,
    pub seen_at: Option<i64>,
}

pub struct Letter {
    pub id: i64,
    pub from: String,
    pub mail: Result<Mail>,
}

impl Letter {
    pub fn rotation(&self) -> bool {
        self.mail.as_ref().is_ok_and(|mail| mail.step == "rotate")
    }
}

impl Peer {
    pub fn public(&self) -> Result<Vec<u8>> {
        let key = self.enc_key.as_deref().and_then(|k| B64.decode(k).ok());
        key.ok_or_else(|| Error::Invalid(format!("{} has no encryption key to share with", self.name)))
    }
}

pub async fn devices(api: &Api) -> Result<Vec<Peer>> {
    let found = api.get("/api/v1/me/devices").await?;
    let seconds = |value: &Value| value.as_i64().map(|ms| ms / 1000);
    let peer = |d: &Value| Peer {
        id: d["id"].as_str().unwrap_or_default().into(),
        name: d["name"].as_str().unwrap_or_default().into(),
        platform: d["platform"].as_str().unwrap_or_default().into(),
        enc_key: d["enc_key"].as_str().map(String::from),
        created_at: seconds(&d["created_at"]).unwrap_or_default(),
        seen_at: seconds(&d["seen_at"]),
    };
    Ok(found["items"].as_array().into_iter().flatten().map(peer).collect())
}

pub async fn inbox(api: &Api) -> Result<Vec<Letter>> {
    let found = api.fetch("/api/v1/mailbox", MAIL).await?;
    let items = found["items"].as_array().into_iter().flatten().filter(|m| m["kind"] == "key-share");
    let letter = |m: &Value| {
        let body = B64.decode(m["body"].as_str().unwrap_or_default()).unwrap_or_default();
        let (id, from) =
            (m["id"].as_i64().unwrap_or_default(), m["from_device"].as_str().unwrap_or_default());
        Letter { id, from: from.to_owned(), mail: Mail::read(&body) }
    };
    Ok(items.map(letter).collect())
}

pub async fn ack(api: &Api, id: i64) -> Result<()> {
    api.call(Method::DELETE, &format!("/api/v1/mailbox/{id}"), None).await.map(drop)
}

pub async fn post(api: &Api, to: &str, body: &[u8]) -> Result<()> {
    let body = json!({"to_device": to, "kind": "key-share", "body": B64.encode(body)});
    api.post("/api/v1/mailbox", body).await.map(drop)
}

pub async fn from(api: &Api, peer: &str) -> Result<Vec<Letter>> {
    let found: Vec<Letter> =
        inbox(api).await?.into_iter().filter(|l| l.from == peer && !l.rotation()).collect();
    for letter in &found {
        ack(api, letter.id).await?;
    }
    Ok(found)
}

pub async fn next(api: &Api, peer: &Peer, pair: &str, step: &str) -> Result<Option<Mail>> {
    let Some(letter) = from(api, &peer.id).await?.into_iter().next() else { return Ok(None) };
    let mail = letter.mail?;
    let name = &peer.name;
    match mail.step.as_str() {
        _ if mail.pair != pair => Err(Error::Invalid(format!(
            "{name} sent a message of another comparison, replayed or out of order: start again"
        ))),
        "abort" => Err(Error::Invalid(format!("{name} stopped the comparison: the key was not shared"))),
        found if found == step => Ok(Some(mail)),
        found => Err(Error::Invalid(format!("{name} sent {found} instead of {step}: start again"))),
    }
}

pub fn nonce(mail: &Mail) -> Result<[u8; 32]> {
    mail.bytes()?.try_into().map_err(|_| Error::Invalid("a device message is broken".into()))
}
