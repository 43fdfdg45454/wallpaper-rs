use crate::repo::{author_id, hex64};
use crate::{Blob, Entry, Error, Key, KeyRevocation, Result, ensure, fingerprint, require, threshold};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const AUTHORS_FORMAT: &str = "wpak-repo-authors/1.0";
pub const ONLINE: &str = "online";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authors {
    pub format: String,
    pub repo_id: String,
    pub authors: Vec<OnlineAuthor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnlineAuthor {
    pub id: String,
    pub keys: BTreeMap<String, Key>,
    pub rotations: Vec<Rotation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rotation {
    pub from: String,
    pub to: String,
    pub sig: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trusted {
    pub repo_id: String,
    pub id: String,
    pub fingerprint: String,
    pub key: String,
}

pub fn rotation_text(author: &str, from: &str, to: &str) -> String {
    format!("wpak-author-rotate {author} {from} {to}")
}

impl Authors {
    pub fn check(&self, repo_id: &str) -> Result<()> {
        let own = self.format == AUTHORS_FORMAT && self.repo_id == repo_id;
        require(own, "authors", "expected a wpak-repo-authors/1.0 of this repository")?;
        let mut ids = BTreeSet::new();
        let fine = |a: &OnlineAuthor| {
            let keys = a.keys.iter().all(|(fp, k)| fingerprint(&k.minisign).is_ok_and(|f| f == *fp));
            let steps = a.rotations.iter().all(|r| hex64(&r.from) && a.keys.contains_key(&r.to));
            author_id(&a.id) && !a.keys.is_empty() && keys && steps
        };
        require(self.authors.iter().all(|a| fine(a) && ids.insert(&a.id)), "authors", "expected unique ids")
    }

    pub fn parse(entry: &Entry, repo_id: &str, blobs: &BTreeMap<String, Vec<u8>>) -> Result<Option<Self>> {
        let Some(blob) = &entry.authors else { return Ok(None) };
        let bytes = blobs.get(&blob.sha256).ok_or_else(|| Error::Missing(blob.sha256.clone()))?;
        let authors: Authors = serde_json::from_slice(bytes)?;
        authors.check(repo_id).map(|()| Some(authors))
    }

    pub fn find(&self, id: &str) -> Option<&OnlineAuthor> {
        self.authors.iter().find(|a| a.id == id)
    }
}

impl OnlineAuthor {
    pub fn material<'a>(&'a self, revoked: &[KeyRevocation]) -> Vec<(&'a str, &'a str)> {
        let live = self.keys.iter().filter(|(fp, _)| revoked.iter().all(|r| r.fingerprint != **fp));
        live.map(|(fp, k)| (fp.as_str(), k.minisign.as_str())).collect()
    }

    pub fn reaches(&self, anchor: &Trusted, to: &str, revoked: &[KeyRevocation]) -> bool {
        let (mut at, mut key, mut seen) = (anchor.fingerprint.clone(), anchor.key.clone(), BTreeSet::new());
        while at != to && seen.insert(at.clone()) {
            let live = |fp: &str| revoked.iter().all(|r| r.fingerprint != fp);
            let text = |r: &Rotation| rotation_text(&self.id, &r.from, &r.to);
            let signed = |r: &&Rotation| {
                r.from == at
                    && live(&at)
                    && threshold(text(r).as_bytes(), &r.sig, &text(r), &[(&at, &key)], 1).is_ok()
            };
            let Some(step) = self.rotations.iter().find(signed) else { return false };
            (at, key) = (step.to.clone(), self.keys[&step.to].minisign.clone());
        }
        at == to
    }
}

pub(crate) fn blob(entry: &Entry) -> Option<(&'static str, &Blob)> {
    entry.authors.as_ref().map(|b| ("authors", b))
}

pub(crate) fn trust(
    anchors: &mut Vec<Trusted>,
    new: Trusted,
    author: &OnlineAuthor,
    revoked: &[KeyRevocation],
) -> Result<()> {
    let found = anchors.iter_mut().find(|a| a.repo_id == new.repo_id && a.id == new.id);
    match found {
        None => anchors.push(new),
        Some(old) if old.fingerprint == new.fingerprint => {}
        Some(old) => {
            let handed = author.reaches(old, &new.fingerprint, revoked);
            ensure(handed, || {
                Error::Signature(format!(
                    "{} is signed by another key than its first installed version, and that key did not hand it over",
                    new.id
                ))
            })?;
            *old = new;
        }
    }
    Ok(())
}
