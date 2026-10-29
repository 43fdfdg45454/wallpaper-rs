use crate::admin::{latest, load, root_path};
use crate::catalog::Catalog;
use crate::repo::at;
use crate::store::{json, write};
use crate::{
    Blob, DETAILED_FORMAT, DETAILS_FORMAT, Details, ENTRY_FORMAT, Entry, KINDS_FORMAT, Listing, Pin,
    REVOKED_FORMAT, Release, Result, Revoked, SHARD_FORMAT, Shard, Signer, Time, comment, require, sha256,
    sha256_file, shard_of, status,
};
use chrono::TimeDelta;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Published {
    pub version: u64,
    pub timestamp: Time,
    pub expires: Time,
    pub packages: usize,
    pub shards: usize,
}

pub fn publish(dir: &Path, signer: &Signer, lifetime_days: u32, now: i64) -> Result<Published> {
    require(status(dir)?.complete, "root", "is not signed by enough root keys")?;
    let (repo_id, version) = latest(dir)?;
    let root = load(dir, version)?;
    let online = root.roles.online.keys.contains(&signer.fingerprint);
    require(online, "key", "is not an online key of the current root")?;
    let allowed = (1..=root.policy.entry_max_days).contains(&lifetime_days);
    require(allowed, "lifetime", "must be between 1 day and the entry_max_days of the root")?;
    let catalog = Catalog::load(dir)?;
    let (packages, shard_bits) = (catalog.packages.len(), catalog.shard_bits);
    let (shards, detailed, revoked) = blobs(dir, &repo_id, catalog)?;
    let last = fs::read_to_string(dir.join("entry/latest")).ok().and_then(|t| t.trim().parse::<u64>().ok());
    let previous: Option<Entry> = last.map(|n| json(&dir.join(format!("entry/{n}.json")))).transpose()?;
    let timestamp = at(now)?;
    require(previous.is_none_or(|p| p.timestamp <= timestamp), "now", "is earlier than the last entry")?;
    let (number, expires) = (last.unwrap_or(0) + 1, timestamp + TimeDelta::days(lifetime_days.into()));
    let root_pin = Pin { version, sha256: sha256_file(root_path(dir, version))? };
    let entry = Entry {
        format: ENTRY_FORMAT.into(),
        repo_id: repo_id.clone(),
        version: number,
        timestamp,
        expires,
        root: root_pin,
        shard_bits,
        shards,
        revoked,
    };
    entry.check(number, &repo_id, &root, &entry.root.sha256)?;
    let bytes = serde_json::to_vec(&entry)?;
    let mut docs = vec![("entry", bytes.clone())];
    if let Some((shards, kinds)) = detailed {
        let (format, entry) = (DETAILS_FORMAT.into(), sha256(&bytes[..])?);
        let details = Details { format, repo_id: repo_id.clone(), version: number, entry, shards, kinds };
        docs.push(("details", serde_json::to_vec(&details)?));
    }
    for (kind, bytes) in docs {
        let sig = signer.sign(&bytes, &comment(kind, number, &repo_id))?;
        write(&dir.join(format!("{kind}/{number}.json")), &bytes, false)?;
        write(&dir.join(format!("{kind}/{number}.json.minisig")), sig.as_bytes(), false)?;
    }
    write(&dir.join("entry/latest"), format!("{number}\n").as_bytes(), false)?;
    Ok(Published { version: number, timestamp, expires, packages, shards: 1 << shard_bits })
}

type Blobs = (Vec<Blob>, Option<(Vec<Blob>, Vec<Blob>)>, Blob);

fn blobs(dir: &Path, repo_id: &str, catalog: Catalog) -> Result<Blobs> {
    let put = |kind: &str, bytes: Vec<u8>| -> Result<Blob> {
        let sha256 = sha256(&bytes[..])?;
        let path = dir.join(kind).join(format!("{sha256}.json"));
        if !path.exists() {
            write(&path, &bytes, false)?;
        }
        Ok(Blob { sha256, size: bytes.len() as u64 })
    };
    let bits = catalog.shard_bits;
    let mut groups = vec![Vec::new(); 1 << bits];
    catalog.packages.iter().for_each(|l| groups[shard_of(&l.id, bits)].push(l));
    let shards = |format: &str, keep: fn(&Release) -> bool| {
        let shards = groups.iter().enumerate().map(|(shard, group)| {
            let packages = group.iter().filter_map(|l| {
                let versions: Vec<_> = l.versions.iter().filter(|r| keep(r)).cloned().collect();
                (!versions.is_empty()).then(|| Listing { versions, ..(*l).clone() })
            });
            let packages = packages.collect();
            let shard = Shard { format: format.into(), repo_id: repo_id.into(), shard, packages };
            shard.check(format, repo_id, shard.shard, bits)?;
            put("index", serde_json::to_vec(&shard)?)
        });
        shards.collect::<Result<Vec<_>>>()
    };
    let releases = || catalog.packages.iter().flat_map(|l| &l.versions);
    let (newer, kinds) = (releases().any(|r| !r.first()), releases().any(|r| !r.kind.first()));
    let detailed = newer.then(|| shards(DETAILED_FORMAT, |r| r.kind.first())).transpose()?;
    let kinds = kinds.then(|| shards(KINDS_FORMAT, |_| true)).transpose()?.unwrap_or_default();
    let detailed = detailed.map(|shards| (shards, kinds));
    let first = shards(SHARD_FORMAT, Release::first)?;
    let (packages, keys) = (catalog.revoked, catalog.keys);
    let revoked = Revoked { format: REVOKED_FORMAT.into(), repo_id: repo_id.into(), packages, keys };
    Ok((first, detailed, put("revoked", serde_json::to_vec(&revoked)?)?))
}
