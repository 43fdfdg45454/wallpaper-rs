use crate::catalog::Catalog;
use crate::store::{json, write};
use crate::{Result, Root, Settings, comment, require, sha256_file, verify_blocks};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RootStatus {
    pub repo_id: String,
    pub version: u64,
    pub signers: Vec<String>,
    pub threshold: usize,
    pub complete: bool,
    pub query: String,
}

pub fn init(dir: &Path, settings: &Settings, now: i64) -> Result<RootStatus> {
    require(!root_path(dir, 1).exists(), "dir", "already holds a repository")?;
    write(&root_path(dir, 1), &serde_json::to_vec(&settings.root(1, None, now)?)?, false)?;
    Catalog { shard_bits: settings.shard_bits, ..Catalog::default() }.save(dir)?;
    status(dir)
}

pub fn rotate_root(dir: &Path, settings: &Settings, now: i64) -> Result<RootStatus> {
    require(status(dir)?.complete, "root", "the current root must be fully signed before rotating")?;
    let (repo_id, version) = latest(dir)?;
    let root = settings.root(version + 1, Some(repo_id), now)?;
    write(&root_path(dir, version + 1), &serde_json::to_vec(&root)?, false)?;
    Catalog { shard_bits: settings.shard_bits, ..Catalog::load(dir)? }.save(dir)?;
    status(dir)
}

#[cfg(feature = "sign")]
pub fn sign_root(dir: &Path, signer: &crate::Signer) -> Result<RootStatus> {
    let (repo_id, version) = latest(dir)?;
    let previous = (version > 1).then(|| load(dir, version - 1)).transpose()?;
    let roots = [Some(load(dir, version)?), previous];
    let allowed = roots.iter().flatten().any(|r| r.roles.root.keys.contains(&signer.fingerprint));
    require(allowed, "key", "is not a root key of this or the previous root")?;
    if !status(dir)?.signers.contains(&signer.fingerprint) {
        let mut sigs = fs::read_to_string(sig_path(dir, version)).unwrap_or_default();
        let data = fs::read(root_path(dir, version))?;
        sigs.push_str(&signer.sign(&data, &comment("root", version, &repo_id))?);
        write(&sig_path(dir, version), sigs.as_bytes(), false)?;
    }
    status(dir)
}

pub fn status(dir: &Path) -> Result<RootStatus> {
    let (repo_id, version) = latest(dir)?;
    let data = fs::read(root_path(dir, version))?;
    let sigs = fs::read_to_string(sig_path(dir, version)).unwrap_or_default();
    let text = comment("root", version, &repo_id);
    let signed =
        |r: &Root| verify_blocks(&data, &sigs, &text, &r.material(&r.roles.root.keys)).unwrap_or_default();
    let mut roots = vec![load(dir, version)?];
    if version > 1 {
        roots.push(load(dir, version - 1)?);
    }
    let sets: Vec<BTreeSet<String>> = roots.iter().map(signed).collect();
    let complete = roots.iter().zip(&sets).all(|(r, s)| s.len() >= r.roles.root.threshold);
    let anchor = load(dir, 1)?.roles.root.keys.first().cloned().unwrap_or_default();
    Ok(RootStatus {
        query: format!("fingerprint={anchor}&repo_id={repo_id}"),
        signers: sets.into_iter().flatten().collect::<BTreeSet<_>>().into_iter().collect(),
        threshold: roots[0].roles.root.threshold,
        repo_id,
        version,
        complete,
    })
}

pub(crate) fn latest(dir: &Path) -> Result<(String, u64)> {
    let repo_id = sha256_file(root_path(dir, 1))?;
    Ok((repo_id, (1..).take_while(|v| root_path(dir, *v).exists()).last().unwrap_or(1)))
}

pub(crate) fn load(dir: &Path, version: u64) -> Result<Root> {
    json(&root_path(dir, version))
}

pub(crate) fn root_path(dir: &Path, version: u64) -> PathBuf {
    dir.join("root").join(format!("{version}.json"))
}

fn sig_path(dir: &Path, version: u64) -> PathBuf {
    dir.join("root").join(format!("{version}.json.minisig"))
}
