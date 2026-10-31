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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gone: BTreeMap<Key, u64>,
}

#[derive(Debug, Default)]
pub struct Synced {
    pub applied: Vec<(Key, Option<View>)>,
    pub copies: Vec<(Key, View)>,
    pub left: usize,
}

pub const LIFETIME_MS: u64 = 90 * 86_400_000;

fn expired(replica: &Replica, now: &Hlc) -> Vec<(Key, i64, Doc)> {
    let old = |doc: &Doc| doc.gone.as_ref().is_some_and(|g| g.0.saturating_add(LIFETIME_MS) < now.0);
    let library = replica.bases.iter().filter(|(key, _)| key.starts_with("library/"));
    let dead = library.filter(|(_, (_, doc))| doc.view().is_none() && !doc.is_cleared() && old(doc));
    dead.map(|(key, (seq, _))| (key.clone(), *seq, Doc::cleared())).collect()
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
    let (deleted, mut left) = (replica.gone.clone(), 0);
    let rule = |key: &str| {
        let mut rule = rules(key.split('/').next().unwrap_or_default());
        rule.keep_absent &= !deleted.contains_key(key);
        rule
    };
    for _ in 0..3 {
        let mut pushes = Vec::new();
        for key in &keys {
            let step = step(replica.bases.get(key), pending.get(key), work.get(key), rule(key), now);
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
        left = 0;
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
                    left += 1;
                }
            }
        }
    }
    let expired = expired(replica, now);
    for ((key, _, doc), outcome) in expired.iter().zip(remote.push(&expired).await?) {
        if let Outcome::Ok(seq) = outcome {
            replica.bases.insert(key.clone(), (seq, doc.clone()));
        }
    }
    let recorded = |key: &Key| replica.bases.get(key).is_some_and(|(_, doc)| doc.view().is_some());
    let gone = std::mem::take(&mut replica.gone);
    replica.gone = gone
        .into_iter()
        .filter(|(key, at)| at + LIFETIME_MS > now.0 && !work.contains_key(key) && recorded(key))
        .collect();
    Ok(Synced { applied: applied.into_iter().collect(), copies, left })
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
