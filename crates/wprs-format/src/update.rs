use crate::entry::contents;
use crate::fetch::{bases, first, get};
use crate::store::{Doc, Snapshot, State};
use crate::{Anchor, Entry, Error, Fetch, Result, Root, comment, ensure, require, sha256};
use std::collections::BTreeMap;
use std::io;

const MAX_ROOT: u64 = 512 << 10;
const MAX_SIG: u64 = 16 << 10;
const MAX_ENTRY: u64 = 128 << 10;
const MAX_LATEST: u64 = 32;
const MAX_ROOTS: usize = 64;
const MAX_PROBES: usize = 16;
const SKEW: i64 = 86_400;

struct Run<'a> {
    fetch: &'a dyn Fetch,
    state: State,
    t: i64,
}

pub(crate) fn refresh(
    fetch: &dyn Fetch,
    state: State,
    root: Option<Doc<Root>>,
    cache: &BTreeMap<String, Vec<u8>>,
    anchor: Option<&Anchor>,
    now: i64,
) -> Result<Snapshot> {
    let mut run = Run { fetch, t: now.max(state.max_timestamp), state };
    let root = match (root, anchor) {
        (Some(root), _) => root,
        (None, Some(anchor)) => run.genesis(anchor)?,
        (None, None) => return Err(Error::Missing("root/1.json".into())),
    };
    let root = run.chain(root, now)?;
    let frozen = format!("root {} (possible freeze attack)", root.value.version);
    ensure(root.value.expires.timestamp() > run.t, || Error::Expired(frozen))?;
    let bases = bases(&run.state, &root.value);
    let entry =
        first(&bases, |base| run.entry(&root, base))?.ok_or_else(|| Error::Missing("entry".into()))?;
    let wanted = entry.value.shards.iter().map(|b| ("index", b)).chain([("revoked", &entry.value.revoked)]);
    let blobs = wanted.map(|(dir, blob)| {
        if let Some(bytes) = cache.get(&blob.sha256).filter(|b| blob.matches(b).is_ok()) {
            return Ok((blob.sha256.clone(), bytes.clone()));
        }
        let path = format!("{dir}/{}.json", blob.sha256);
        let fetched = first(&bases, |base| {
            let bytes = get(fetch, base, &path, blob.size)?;
            bytes.map(|b| blob.matches(&b).map(|()| b)).transpose()
        })?;
        Ok((blob.sha256.clone(), fetched.ok_or(Error::Missing(path))?))
    });
    let blobs = blobs.collect::<Result<BTreeMap<_, _>>>()?;
    let (shards, revoked) = contents(&entry.value, &run.state.repo_id, &blobs)?;
    let (mut state, timestamp) = (run.state, entry.value.timestamp.timestamp());
    state.root_version = root.value.version;
    state.entry_version = entry.value.version;
    state.entry_sha256 = sha256(&entry.json[..])?;
    state.entry_timestamp = timestamp;
    state.max_timestamp = state.max_timestamp.max(timestamp);
    state.stale = entry.value.expires.timestamp() <= run.t;
    state.checked = now;
    Ok(Snapshot { state, root, entry, blobs, shards, revoked })
}

impl Run<'_> {
    fn signed(&self, base: &str, path: &str, max: u64) -> Result<Option<(Vec<u8>, String)>> {
        let Some(json) = get(self.fetch, base, path, max)? else { return Ok(None) };
        let sig = get(self.fetch, base, &format!("{path}.minisig"), MAX_SIG)?;
        let sig = String::from_utf8(sig.ok_or_else(|| Error::Missing(format!("{path}.minisig")))?);
        Ok(Some((json, sig.map_err(|_| Error::Signature(format!("{path}.minisig is not UTF-8")))?)))
    }

    fn genesis(&mut self, anchor: &Anchor) -> Result<Doc<Root>> {
        let root = first(&self.state.bases, |base| {
            let Some((json, sig)) = self.signed(base, "root/1.json", MAX_ROOT)? else { return Ok(None) };
            let id = sha256(&json[..])?;
            ensure(anchor.repo_id.as_ref().is_none_or(|r| *r == id), || {
                Error::Mismatch("root/1.json".into())
            })?;
            let root: Root = serde_json::from_slice(&json)?;
            root.check(1, &id)?;
            let signers = root.meets(&root.roles.root, &json, &sig, &comment("root", 1, &id))?;
            let anchored = anchor.fingerprint.as_ref().is_none_or(|fp| signers.contains(fp));
            ensure(anchored, || Error::Signature("root/1.json is not signed by the anchor key".into()))?;
            Ok(Some(Doc { value: root, json, sig }))
        })?;
        let root = root.ok_or_else(|| Error::Missing("root/1.json".into()))?;
        self.state.repo_id = sha256(&root.json[..])?;
        Ok(root)
    }

    fn chain(&mut self, mut root: Doc<Root>, now: i64) -> Result<Doc<Root>> {
        let id = self.state.repo_id.clone();
        for _ in 0..MAX_ROOTS {
            let version = root.value.version + 1;
            let next = first(&bases(&self.state, &root.value), |base| {
                let path = format!("root/{version}.json");
                let Some((json, sig)) = self.signed(base, &path, MAX_ROOT)? else { return Ok(None) };
                let new: Root = serde_json::from_slice(&json)?;
                new.check(version, &id)?;
                let text = comment("root", version, &id);
                root.value.meets(&root.value.roles.root, &json, &sig, &text)?;
                new.meets(&new.roles.root, &json, &sig, &text)?;
                Ok(Some(Doc { value: new, json, sig }))
            })?;
            let Some(next) = next else { break };
            root = next;
        }
        if root.value.roles.online.keys != self.state.online_keys {
            let reset = State {
                online_keys: root.value.roles.online.keys.clone(),
                max_timestamp: now,
                ..State::default()
            };
            self.state =
                State { url: self.state.url.clone(), bases: self.state.bases.clone(), repo_id: id, ..reset };
        }
        Ok(root)
    }

    fn entry(&self, root: &Doc<Root>, base: &str) -> Result<Option<Doc<Entry>>> {
        let Some(latest) = get(self.fetch, base, "entry/latest", MAX_LATEST)? else { return Ok(None) };
        let latest = std::str::from_utf8(&latest).ok().and_then(|text| text.trim().parse::<u64>().ok());
        let mut version = latest.ok_or_else(|| Error::Invalid {
            field: "entry/latest".into(),
            reason: "expected a decimal number".into(),
        })?;
        version = version.max(self.state.entry_version);
        let Some(mut entry) = self.load_entry(root, base, version)? else { return Ok(None) };
        for _ in 0..MAX_PROBES {
            match version.checked_add(1).map(|v| self.load_entry(root, base, v)) {
                Some(Ok(Some(next))) => (entry, version) = (next, version + 1),
                Some(Err(Error::Io(e))) if e.kind() == io::ErrorKind::Interrupted => return Err(e.into()),
                _ => break,
            }
        }
        Ok(Some(entry))
    }

    fn load_entry(&self, root: &Doc<Root>, base: &str, version: u64) -> Result<Option<Doc<Entry>>> {
        let path = format!("entry/{version}.json");
        let Some((json, sig)) = self.signed(base, &path, MAX_ENTRY)? else { return Ok(None) };
        let (state, current) = (&self.state, &root.value);
        current.meets(&current.roles.online, &json, &sig, &comment("entry", version, &state.repo_id))?;
        let entry: Entry = serde_json::from_slice(&json)?;
        entry.check(version, &state.repo_id, current, &sha256(&root.json[..])?)?;
        let same = version != state.entry_version || sha256(&json[..])? == state.entry_sha256;
        ensure(same, || Error::Equivocation(version))?;
        let timestamp = entry.timestamp.timestamp();
        ensure(timestamp >= state.entry_timestamp, || {
            Error::Rollback(format!("{path} is older than the last entry"))
        })?;
        require(timestamp <= self.t + SKEW, "entry.timestamp", "is more than a day in the future")?;
        Ok(Some(Doc { value: entry, json, sig }))
    }
}
