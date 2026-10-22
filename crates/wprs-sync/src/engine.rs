use crate::{Doc, Error, Hlc, Result, Rules, View, step};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type Key = String;

pub struct Change {
    pub key: Key,
    pub seq: i64,
    pub epoch: i64,
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub held: BTreeMap<Key, (i64, Doc)>,
}

#[derive(Debug, Default)]
pub struct Synced {
    pub applied: Vec<(Key, Option<View>)>,
    pub copies: Vec<(Key, View)>,
}

async fn pull(remote: &impl Remote, replica: &mut Replica) -> Result<BTreeMap<Key, Change>> {
    let (mut since, mut found) = (replica.seq, BTreeMap::new());
    let mut whole = since == 0;
    loop {
        let page = match remote.pull(since).await {
            Err(e) if e.code() == "resync" && since > 0 => {
                (since, whole) = (0, true);
                continue;
            }
            page => page?,
        };
        for change in page.changes {
            found.insert(change.key.clone(), change);
        }
        if !page.more {
            replica.seq = page.seq;
            if whole {
                replica.bases.retain(|key, _| found.contains_key(key));
            }
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
) -> Result<Synced> {
    let mut pending: BTreeMap<Key, (i64, Doc)> =
        pull(remote, replica).await?.into_iter().map(|(key, c)| (key, (c.seq, c.doc))).collect();
    for (key, held) in std::mem::take(&mut replica.held) {
        pending.entry(key).or_insert(held);
    }
    let (mut work, mut applied, mut copies) = (local.clone(), BTreeMap::new(), Vec::new());
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
            copies.extend(step.copy.map(|view| (key.clone(), view)));
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
    Ok(Synced { applied: applied.into_iter().collect(), copies })
}

pub async fn rekey(remote: &impl Remote, epoch: i64) -> Result<usize> {
    let found = pull(remote, &mut Replica::default()).await?;
    let stale = found.into_values().filter(|c| c.epoch < epoch && c.doc != Doc::cleared());
    let stale: Vec<(Key, i64, Doc)> = stale.map(|c| (c.key, c.seq, c.doc)).collect();
    let mut done = 0;
    for chunk in stale.chunks(100) {
        let outcomes = remote.push(chunk).await?;
        done += outcomes.iter().filter(|o| matches!(o, Outcome::Ok(_))).count();
    }
    Ok(done)
}
