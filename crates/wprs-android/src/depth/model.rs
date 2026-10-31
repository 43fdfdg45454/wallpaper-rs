use crate::said;
use crate::{Error, settings};
use serde_json::{Value, json};
use std::fs::{self, File, Permissions};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use wprs_depth::{Asset, MODEL, Part, Places, RUNTIME};

const AGENT: &str = concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION"));
const MAX_LIBRARY: u64 = 256 << 20;

pub(crate) fn assets() -> Vec<Asset> {
    std::iter::once(MODEL).chain(RUNTIME).collect()
}

fn needed(places: &Places) -> Vec<Asset> {
    let found = !places.library(None).is_empty();
    assets()
        .into_iter()
        .filter(|a| places.installed(a).is_none() && !(a.kind == Part::Runtime && found))
        .collect()
}

pub(crate) fn ready(places: &Places) -> Result<(), Error> {
    let missing =
        || said("android.depth_model_missing", "the depth model is not installed: download it first", &[]);
    places.installed(&MODEL).map(drop).ok_or_else(missing)
}

pub(crate) fn part(places: &Places, asset: &Asset) -> PathBuf {
    places.file(asset).with_file_name(asset.archive()).with_added_extension("part")
}

fn joined(assets: &[&Asset], field: impl Fn(&Asset) -> String) -> String {
    let mut seen = std::collections::BTreeSet::new();
    let fields = assets.iter().map(|a| field(a)).filter(|f| seen.insert(f.clone()));
    fields.collect::<Vec<_>>().join(", ")
}

pub(crate) fn shown(places: &Places) -> Value {
    let (all, missing) = (assets(), needed(places));
    let missing: Vec<&Asset> = missing.iter().collect();
    let listed = if missing.is_empty() { all.iter().collect() } else { missing.clone() };
    let files: Vec<Value> = missing
        .iter()
        .map(|a| json!({"urls": a.urls, "bytes": a.bytes, "part": part(places, a), "agent": AGENT}))
        .collect();
    let bytes: u64 = listed.iter().map(|a| a.bytes).sum();
    json!({
        "installed": missing.is_empty(), "allowed": settings::downloads(), "files": files,
        "title": joined(&listed, |a| a.title.into()), "size": format!("{:.1} MB", bytes as f64 / 1e6),
        "hosts": joined(&listed, Asset::hosts), "license": joined(&listed, |a| a.license.into()),
        "license_url": joined(&listed, |a| a.license_url.into()),
    })
}

fn wrong(asset: &Asset) -> Error {
    let hosts = asset.hosts();
    said("android.model_checksum", "the file from %1 has another SHA-256 and was deleted", &[&hosts])
}

pub(crate) fn unpack(asset: &Asset, archive: &Path, dest: &Path) -> Result<(), Error> {
    let found = File::open(archive).ok().and_then(|f| zip::ZipArchive::new(f).ok());
    let mut zip = found.ok_or_else(|| wrong(asset))?;
    let mut entry = zip.by_name(asset.member).map_err(|_| wrong(asset))?;
    let dir = dest.parent().ok_or(Error::Unprepared)?;
    let mut out = tempfile::Builder::new().prefix(".unpack-").tempfile_in(dir)?;
    std::io::copy(&mut (&mut entry).take(MAX_LIBRARY), out.as_file_mut())?;
    out.as_file().sync_all()?;
    if wprs_depth::sha256(out.path()).ok().as_deref() != Some(asset.file_sha256) {
        return Err(wrong(asset));
    }
    out.as_file().set_permissions(Permissions::from_mode(0o444))?;
    out.persist(dest).map(drop).map_err(|e| e.error.into())
}

fn settle(places: &Places, asset: &Asset) -> Result<(), Error> {
    let (file, part) = (places.file(asset), part(places, asset));
    let folder = file.parent().ok_or(Error::Unprepared)?;
    if wprs_depth::sha256(&part).ok().as_deref() != Some(asset.sha256) {
        drop(fs::remove_file(&part));
        return Err(wrong(asset));
    }
    let unpacked = match asset.member {
        "" => fs::rename(&part, &file).map_err(Error::from),
        _ => unpack(asset, &part, &file),
    };
    drop(fs::remove_file(&part));
    unpacked?;
    fs::write(folder.join("NOTICE.txt"), asset.notice())?;
    Ok(fs::write(folder.join("LICENSE.txt"), asset.license_text)?)
}

pub(crate) fn install(places: &Places) -> Result<Value, Error> {
    if !settings::downloads() {
        assets().iter().for_each(|a| drop(fs::remove_file(part(places, a))));
        return Err(said("android.download_off", "downloading the model is turned off", &[]));
    }
    needed(places).iter().try_for_each(|asset| settle(places, asset))?;
    Ok(Value::Null)
}

pub(crate) fn forget(places: &Places) -> bool {
    let dirs = assets().into_iter().filter_map(|a| Some(places.file(&a).parent()?.to_path_buf()));
    dirs.map(|dir| fs::remove_dir_all(dir).is_ok()).fold(false, |a, b| a | b)
}
