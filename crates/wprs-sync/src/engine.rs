use crate::{Doc, Error, Hlc, Result, Rules, View, step};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type Key = String;

pub struct Change {
    pub key: Key,
    pub seq: i64,
    pub doc: Doc,
}

pub struct Page {
    pub seq: i64,
    pub more: bool,
    pub last: i64,
    pub changes: Vec<Change>,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Ok(i64),
    Conflict(Option<(i64, Doc)>),
}

pub trait Remote {
    fn pull(&self, since: i64) -> impl Future<Output = Result<Page>>;
    fn push(&self, batch: &[(Key, i64, Doc)]) -> impl Future<Output = Result<Vec<Outcome>>>;
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Replica {
    pub seq: i64,
    pub bases: BTreeMap<Key, (i64, Doc)>,
}

async fn pull(remote: &impl Remote, replica: &mut Replica) -> Result<BTreeMap<Key, (i64, Doc)>> {
    let (mut since, mut found) = (replica.seq, BTreeMap::new());
    loop {
        let page = match remote.pull(since).await {
            Err(e) if e.code() == "resync" && since > 0 => {
                since = 0;
                continue;
            }
            page => page?,
        };
        for change in page.changes {
            found.insert(change.key, (change.seq, change.doc));
        }
        if !page.more {
            replica.seq = page.seq;
            return Ok(found);
        }
        if page.last <= since {
            return Err(Error::Invalid("the server sent the same page of changes again".into()));
        }
        since = page.last;
    }
}

pub async fn sync(
    remote: &impl Remote,
    replica: &mut Replica,
    local: &BTreeMap<Key, View>,
    rules: impl Fn(&str) -> Rules,
    now: &Hlc,
) -> Result<Vec<(Key, Option<View>)>> {
    let mut pending = pull(remote, replica).await?;
    let (mut work, mut applied) = (local.clone(), BTreeMap::new());
    let mut keys: BTreeSet<Key> =
        replica.bases.keys().chain(pending.keys()).chain(local.keys()).cloned().collect();
    for _ in 0..3 {
        let mut pushes = Vec::new();
        for key in &keys {
            let collection = key.split('/').next().unwrap_or_default();
            let step = step(replica.bases.get(key), pending.get(key), work.get(key), rules(collection), now);
            if let Some(base) = step.base {
                replica.bases.insert(key.clone(), base);
            }
            if let Some(view) = step.apply {
                match &view {
                    Some(view) => work.insert(key.clone(), view.clone()),
                    None => work.remove(key),
                };
                applied.insert(key.clone(), view);
            }
            pushes.extend(step.push.map(|(seq, doc)| (key.clone(), seq, doc)));
        }
        if pushes.is_empty() {
            break;
        }
        let outcomes = remote.push(&pushes).await?;
        (pending, keys) = (BTreeMap::new(), BTreeSet::new());
        for ((key, _, doc), outcome) in pushes.into_iter().zip(outcomes) {
            match outcome {
                Outcome::Ok(seq) => drop(replica.bases.insert(key, (seq, doc))),
                Outcome::Conflict(current) => {
                    match current {
                        Some(current) => drop(pending.insert(key.clone(), current)),
                        None => drop(replica.bases.remove(&key)),
                    }
                    keys.insert(key);
                }
            }
        }
    }
    Ok(applied.into_iter().collect())
}
