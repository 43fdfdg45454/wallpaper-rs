use crate::admin::{latest, load};
use crate::repo::at;
use crate::store::{json, place, write};
use crate::validate::{extension, within};
use crate::{
    Error, KeyRevocation, Kind, Limits, Listing, MANIFEST, Needs, OnlineAuthor, Package, Preview, Rated,
    Reason, Release, Result, Revocation, Root, SIGNATURE, ensure, open, require, sha256, sha256_file,
    verify_package,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<OnlineAuthor>,
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
    package.manifest.publishable()?;
    package.look_closer()?;
    let author = signer(&mut package, &root, author, &catalog)?;
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
    let hidden = listing.versions.iter().any(|r| r.hidden);
    listing.versions.push(Release { hidden, ..added.release.clone() });
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

pub fn unrevoke(dir: &Path, id: &str, version: &str) -> Result<()> {
    let mut catalog = Catalog::load(dir)?;
    catalog.revoked.retain(|r| r.id != id || r.versions != [version]);
    catalog.save(dir)
}

pub fn hide(dir: &Path, id: &str, hidden: bool) -> Result<()> {
    let mut catalog = Catalog::load(dir)?;
    let listing = catalog.packages.iter_mut().find(|l| l.id == id);
    let listing = listing.ok_or_else(|| Error::NotFound(id.into()))?;
    listing.versions.iter_mut().for_each(|r| r.hidden = hidden);
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
    catalog: &Catalog,
) -> Result<Option<String>> {
    let revoked = &catalog.keys;
    let wanted = |id: &str| author.is_none_or(|a| a == id);
    let rooted =
        root.authors.iter().filter(|a| wanted(&a.id)).map(|a| (a.id.clone(), root.author_keys(a, revoked)));
    let online = catalog.authors.iter().filter(|a| root.policy.online() && wanted(&a.id));
    let online = online
        .filter(|a| root.authors.iter().all(|r| r.id != a.id))
        .map(|a| (a.id.clone(), a.material(revoked)));
    let candidates: Vec<_> = rooted.chain(online).collect();
    require(author.is_none() || !candidates.is_empty(), "author", "is not listed in the root")?;
    if !package.entries.iter().any(|e| e.path == SIGNATURE) {
        let optional = author.is_none() && !root.policy.require_author_signature;
        ensure(optional, || Error::Signature("the package must carry an author signature".into()))?;
        return Ok(None);
    }
    let found = candidates.into_iter().find(|(_, keys)| verify_package(package, keys).is_ok());
    let found = found.ok_or_else(|| Error::Signature("no author key of this repository signed it".into()))?;
    Ok(Some(found.0))
}

fn release(package: &mut Opened, dir: &Path, wpak: &Path, size: u64, now: i64) -> Result<Release> {
    let manifest_sha256 = sha256(&package.read_file(MANIFEST)?[..])?;
    let manifest = package.manifest.localized();
    let mut copy = |path: Option<&String>| -> Result<Option<Preview>> {
        let Some(path) = path else { return Ok(None) };
        let bytes = package.read_file(path)?;
        let (sha256, ext) = (sha256(&bytes[..])?, extension(path).unwrap_or_default());
        write(&dir.join("previews").join(format!("{sha256}.{ext}")), &bytes, false)?;
        Ok(Some(Preview { sha256, size: bytes.len() as u64, ext }))
    };
    let (preview, animated) = (copy(manifest.thumbnail())?, copy(manifest.animated())?);
    let v2 = manifest.v2().then_some(&manifest);
    let facets = if manifest.v2() { package.facts.facets.clone() } else { Vec::new() };
    let base = (manifest.kind == Kind::Preset).then(|| package.preset().map(|p| p.base.id)).transpose()?;
    Ok(Release {
        version: manifest.version.clone(),
        kind: manifest.kind,
        title: manifest.title.clone(),
        description: manifest.description.clone(),
        tags: manifest.tags.clone(),
        license: manifest.license.clone(),
        authors: manifest.authors.clone(),
        size,
        sha256: sha256_file(wpak)?,
        manifest_sha256,
        preview,
        published: at(now)?,
        schema: v2.map(|m| m.schema.clone()).unwrap_or_default(),
        default_locale: manifest.default_locale.clone(),
        changes: manifest.changes.clone(),
        content: manifest.content.as_ref().map(Rated::from),
        photosensitive: manifest.photosensitive,
        requires: v2.and_then(|m| m.requires.as_ref()).map(Needs::from),
        facets,
        animated,
        platforms: manifest.platforms.iter().map(|p| p.name().to_owned()).collect(),
        base,
        hidden: false,
    })
}
