use crate::admin::{latest, load};
use crate::repo::at;
use crate::store::{json, place, write};
use crate::validate::{extension, within};
use crate::{
    Error, KeyRevocation, Limits, Listing, MANIFEST, Package, Preview, Reason, Release, Result, Revocation,
    Root, SIGNATURE, ensure, open, require, sha256, sha256_file, verify_package,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::Path;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Catalog {
    pub shard_bits: u8,
    pub packages: Vec<Listing>,
    pub revoked: Vec<Revocation>,
    pub keys: Vec<KeyRevocation>,
}

impl Catalog {
    pub fn load(dir: &Path) -> Result<Self> {
        json(&dir.join("catalog.json"))
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        write(&dir.join("catalog.json"), &serde_json::to_vec_pretty(self)?, false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Added {
    pub id: String,
    pub author: Option<String>,
    pub release: Release,
}

pub fn add(dir: &Path, wpak: &Path, author: Option<&str>, now: i64) -> Result<Added> {
    let (root, mut catalog) = (load(dir, latest(dir)?.1)?, Catalog::load(dir)?);
    let size = fs::metadata(wpak)?.len();
    within("package size", size, root.policy.max_package_size)?;
    let mut package = open(wpak, &Limits::default())?;
    let author = signer(&mut package, &root, author, &catalog.keys)?;
    let (id, version) = (package.manifest.id.clone(), package.manifest.version.clone());
    let added = Added { id: id.clone(), author, release: release(&mut package, dir, wpak, size, now)? };
    let index = catalog.packages.iter().position(|l| l.id == id).unwrap_or_else(|| {
        catalog.packages.push(Listing { id: id.clone(), author: added.author.clone(), versions: Vec::new() });
        catalog.packages.len() - 1
    });
    let listing = &mut catalog.packages[index];
    require(listing.author == added.author, "author", "differs from the author of earlier versions")?;
    if let Some(old) = listing.versions.iter().find(|r| r.version == version) {
        require(old.sha256 == added.release.sha256, "version", "is already published with other content")?;
        return Ok(Added { release: old.clone(), ..added });
    }
    let dest = dir.join("packages").join(&id).join(format!("{version}.wpak"));
    place(&dest, false, |file| Ok(io::copy(&mut File::open(wpak)?, file).map(drop)?))?;
    listing.versions.push(added.release.clone());
    listing.versions.sort_by_key(|r| semver::Version::parse(&r.version).ok());
    catalog.packages.sort_by(|a, b| a.id.cmp(&b.id));
    catalog.save(dir)?;
    Ok(added)
}

pub fn revoke(dir: &Path, id: &str, version: Option<&str>, reason: Reason, message: &str) -> Result<()> {
    let mut catalog = Catalog::load(dir)?;
    let versions = vec![version.unwrap_or("*").to_string()];
    catalog.revoked.push(Revocation { id: id.into(), versions, reason, message: message.into() });
    catalog.save(dir)
}

pub fn revoke_key(dir: &Path, fingerprint: &str, reason: Reason) -> Result<()> {
    let mut catalog = Catalog::load(dir)?;
    catalog.keys.push(KeyRevocation { fingerprint: fingerprint.into(), reason });
    catalog.save(dir)
}

type Opened = Package<BufReader<File>>;

fn signer(
    package: &mut Opened,
    root: &Root,
    author: Option<&str>,
    revoked: &[KeyRevocation],
) -> Result<Option<String>> {
    let candidates: Vec<_> = root.authors.iter().filter(|a| author.is_none_or(|id| id == a.id)).collect();
    require(author.is_none() || !candidates.is_empty(), "author", "is not listed in the root")?;
    if !package.entries.iter().any(|e| e.path == SIGNATURE) {
        let optional = author.is_none() && !root.policy.require_author_signature;
        ensure(optional, || Error::Signature("the package must carry an author signature".into()))?;
        return Ok(None);
    }
    let found =
        candidates.into_iter().find(|a| verify_package(package, &root.author_keys(a, revoked)).is_ok());
    let found = found.ok_or_else(|| Error::Signature("no author key of this repository signed it".into()))?;
    Ok(Some(found.id.clone()))
}

fn release(package: &mut Opened, dir: &Path, wpak: &Path, size: u64, now: i64) -> Result<Release> {
    let manifest_sha256 = sha256(&package.read_file(MANIFEST)?[..])?;
    let manifest = package.manifest.clone();
    let preview = manifest.preview.as_deref().map(|path| -> Result<Preview> {
        let bytes = package.read_file(path)?;
        let (sha256, ext) = (sha256(&bytes[..])?, extension(path).unwrap_or_default());
        write(&dir.join("previews").join(format!("{sha256}.{ext}")), &bytes, false)?;
        Ok(Preview { sha256, size: bytes.len() as u64, ext })
    });
    Ok(Release {
        version: manifest.version,
        kind: manifest.kind,
        title: manifest.title,
        description: manifest.description,
        tags: manifest.tags,
        license: manifest.license,
        authors: manifest.authors,
        size,
        sha256: sha256_file(wpak)?,
        manifest_sha256,
        preview: preview.transpose()?,
        published: at(now)?,
    })
}
