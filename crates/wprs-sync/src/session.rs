use crate::{Api, Change, Context, Doc, Error, Outcome, PAGE, Page, Remote, Result, SyncKey, open, seal};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

const BATCH_BYTES: usize = 4 << 20;

pub struct Session {
    pub api: Api,
    pub user: i64,
    pub epoch: i64,
    pub encrypted: bool,
    pub key: SyncKey,
    pub old: Option<SyncKey>,
    rejected: Mutex<BTreeMap<String, i64>>,
}

pub fn split(key: &str) -> (&str, &str) {
    key.split_once('/').unwrap_or((key, ""))
}

impl Session {
    pub fn new(api: Api, user: i64, epoch: i64, encrypted: bool, key: SyncKey) -> Session {
        Session { api, user, epoch, encrypted, key, old: None, rejected: Mutex::default() }
    }

    fn context<'a>(&self, key: &'a str, epoch: i64) -> Context<'a> {
        let (collection, item) = split(key);
        Context { user: self.user, epoch, collection, item }
    }

    pub fn encode(&self, key: &str, doc: &Doc) -> Result<String> {
        let json = serde_json::to_vec(doc)?;
        let bytes = match self.encrypted {
            true => seal(&self.key.records(self.epoch), &self.context(key, self.epoch), &json)?,
            false => json,
        };
        Ok(B64.encode(bytes))
    }

    pub fn decode(&self, key: &str, payload: &str, epoch: i64) -> Result<Doc> {
        let bytes = B64.decode(payload).map_err(|_| Error::Invalid("a record is not base64".into()))?;
        let json = match self.encrypted {
            true => {
                let sync = if epoch == self.epoch { Some(&self.key) } else { self.old.as_ref() };
                let sync = sync.filter(|_| epoch >= self.epoch - 1 && epoch <= self.epoch);
                let sync = sync.ok_or(Error::Crypto("a record belongs to another key epoch"))?;
                open(&sync.records(epoch), &self.context(key, epoch), &bytes)?
            }
            false => bytes,
        };
        Ok(serde_json::from_slice(&json)?)
    }

    pub fn rejected(&self) -> BTreeMap<String, i64> {
        self.rejected.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    fn record(&self, key: &str, value: &Value) -> Option<Doc> {
        let Some(payload) = value["payload"].as_str().filter(|_| value["deleted"] != true) else {
            return Some(Doc::cleared());
        };
        let epoch = value["epoch"].as_i64().unwrap_or(self.epoch);
        let doc = self.decode(key, payload, epoch).ok();
        if doc.is_none() && epoch <= self.epoch {
            let mut rejected = self.rejected.lock().unwrap_or_else(PoisonError::into_inner);
            rejected.insert(key.to_owned(), value["seq"].as_i64().unwrap_or_default());
        }
        doc
    }

    async fn send(&self, batch: &[(String, i64, Doc)], changes: &[Value]) -> Result<Vec<Outcome>> {
        let body = json!({"epoch": self.epoch, "format": crate::FORMAT, "changes": changes});
        let answer = self.api.post("/api/v1/sync/changes", body);
        let answer = answer.await?;
        let results = answer["results"].as_array().into_iter().flatten();
        let out: Vec<Outcome> = batch
            .iter()
            .zip(results)
            .map(|((key, ..), result)| match (result["status"].as_str(), result["current"]["seq"].as_i64()) {
                (Some("ok"), _) => Outcome::Ok(result["seq"].as_i64().unwrap_or_default()),
                (_, Some(seq)) => {
                    Outcome::Conflict(self.record(key, &result["current"]).map(|doc| (seq, doc)))
                }
                _ => Outcome::Conflict(None),
            })
            .collect();
        if out.len() != batch.len() {
            return Err(Error::Invalid("the server answered fewer results than changes".into()));
        }
        Ok(out)
    }
}

impl Remote for Session {
    async fn pull(&self, since: i64) -> Result<Page> {
        let page = self.api.fetch(&format!("/api/v1/sync/changes?since={since}&limit=500"), PAGE).await?;
        let newer =
            page["changes"].as_array().into_iter().flatten().any(|c| c["epoch"].as_i64() > Some(self.epoch));
        if newer || page["epoch"].as_i64().is_some_and(|epoch| epoch != self.epoch) {
            let detail = "the sync key changed during the sync; it starts again with the new key";
            return Err(Error::Server { status: 409, code: "epoch_mismatch".into(), detail: detail.into() });
        }
        let changes = page["changes"].as_array().into_iter().flatten().filter_map(|c| {
            let text = |field: &str| c[field].as_str().unwrap_or_default().to_owned();
            let key = format!("{}/{}", text("collection"), text("item"));
            let epoch = c["epoch"].as_i64().unwrap_or(self.epoch);
            Some(Change { doc: self.record(&key, c)?, seq: c["seq"].as_i64()?, epoch, key })
        });
        let changes = changes.collect();
        let last = page["changes"].as_array().into_iter().flatten().filter_map(|c| c["seq"].as_i64()).max();
        let (seq, more) = (page["seq"].as_i64().unwrap_or_default(), page["more"] == true);
        Ok(Page { seq, more, last: last.unwrap_or(since), changes })
    }

    async fn push(&self, batch: &[(String, i64, Doc)]) -> Result<Vec<Outcome>> {
        let mut changes = Vec::new();
        for (key, base, doc) in batch {
            let (collection, item) = split(key);
            changes.push(match doc.is_cleared() {
                true => json!({"collection": collection, "item": item, "base": base, "deleted": true}),
                false => json!({"collection": collection, "item": item, "base": base, "payload": self.encode(key, doc)?}),
            });
        }
        let (mut groups, mut size) = (Vec::<(usize, usize)>::new(), 0);
        for (i, change) in changes.iter().enumerate() {
            let len = change["payload"].as_str().map_or(0, str::len);
            match groups.last_mut() {
                Some((start, end)) if i - *start < 500 && size + len <= BATCH_BYTES => {
                    (*end, size) = (i + 1, size + len)
                }
                _ => {
                    groups.push((i, i + 1));
                    size = len;
                }
            }
        }
        let mut out = Vec::new();
        for (start, end) in groups {
            out.extend(self.send(&batch[start..end], &changes[start..end]).await?);
        }
        Ok(out)
    }
}
