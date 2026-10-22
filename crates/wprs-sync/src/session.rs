use crate::{Api, Change, Context, Doc, Error, Outcome, Page, Remote, Result, SyncKey, open, seal};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU32, Ordering};

pub struct Session {
    pub api: Api,
    pub user: i64,
    pub epoch: i64,
    pub encrypted: bool,
    pub key: SyncKey,
    pub rejected: AtomicU32,
}

pub fn split(key: &str) -> (&str, &str) {
    key.split_once('/').unwrap_or((key, ""))
}

impl Session {
    fn context<'a>(&self, key: &'a str) -> Context<'a> {
        let (collection, item) = split(key);
        Context { user: self.user, epoch: self.epoch, collection, item }
    }

    pub fn encode(&self, key: &str, doc: &Doc) -> Result<String> {
        let json = serde_json::to_vec(doc)?;
        let bytes = match self.encrypted {
            true => seal(&self.key.records(self.epoch), &self.context(key), &json)?,
            false => json,
        };
        Ok(B64.encode(bytes))
    }

    pub fn decode(&self, key: &str, payload: &str) -> Result<Doc> {
        let bytes = B64.decode(payload).map_err(|_| Error::Invalid("a record is not base64".into()))?;
        let json = match self.encrypted {
            true => open(&self.key.records(self.epoch), &self.context(key), &bytes)?,
            false => bytes,
        };
        Ok(serde_json::from_slice(&json)?)
    }

    fn record(&self, key: &str, value: &Value) -> Option<Doc> {
        let Some(payload) = value["payload"].as_str().filter(|_| value["deleted"] != true) else {
            return Some(Doc::cleared());
        };
        let doc = self.decode(key, payload).ok();
        self.rejected.fetch_add(u32::from(doc.is_none()), Ordering::Relaxed);
        doc
    }
}

impl Remote for Session {
    async fn pull(&self, since: i64) -> Result<Page> {
        let page = self.api.get(&format!("/api/v1/sync/changes?since={since}&limit=500")).await?;
        let changes = page["changes"].as_array().into_iter().flatten().filter_map(|c| {
            let text = |field: &str| c[field].as_str().unwrap_or_default().to_owned();
            let key = format!("{}/{}", text("collection"), text("item"));
            Some(Change { doc: self.record(&key, c)?, seq: c["seq"].as_i64()?, key })
        });
        let changes = changes.collect();
        let last = page["changes"].as_array().into_iter().flatten().filter_map(|c| c["seq"].as_i64()).max();
        let (seq, more) = (page["seq"].as_i64().unwrap_or_default(), page["more"] == true);
        Ok(Page { seq, more, last: last.unwrap_or(since), changes })
    }

    async fn push(&self, batch: &[(String, i64, Doc)]) -> Result<Vec<Outcome>> {
        let mut out = Vec::new();
        for chunk in batch.chunks(500) {
            let change = |(key, base, doc): &(String, i64, Doc)| {
                let (collection, item) = split(key);
                let payload = self.encode(key, doc)?;
                Ok(
                    json!({"collection": collection, "item": item, "base": base, "deleted": false, "payload": payload}),
                )
            };
            let changes = chunk.iter().map(change).collect::<Result<Vec<_>>>()?;
            let body = json!({"epoch": self.epoch, "changes": changes});
            let answer = self.api.post("/api/v1/sync/changes", body).await?;
            for ((key, ..), result) in chunk.iter().zip(answer["results"].as_array().into_iter().flatten()) {
                let current = &result["current"];
                out.push(match (result["status"].as_str(), current["seq"].as_i64()) {
                    (Some("ok"), _) => Outcome::Ok(result["seq"].as_i64().unwrap_or_default()),
                    (_, Some(seq)) => Outcome::Conflict(self.record(key, current).map(|doc| (seq, doc))),
                    _ => Outcome::Conflict(None),
                });
            }
        }
        if out.len() != batch.len() {
            return Err(Error::Invalid("the server answered fewer results than changes".into()));
        }
        Ok(out)
    }
}
