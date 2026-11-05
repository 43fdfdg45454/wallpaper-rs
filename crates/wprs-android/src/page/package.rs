use crate::pack::Pack;
use crate::{Error, said};
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_format::{Input, Kind, Package};
use wprs_scene::VOLUME;

const DOMAIN: &str = "wpak.invalid";

pub(crate) struct Web {
    pub(crate) wpak: PathBuf,
    pub(crate) entry: String,
    pub(crate) entries: HashSet<String>,
    pub(crate) preload: Option<String>,
    pub(crate) background: String,
    pub(crate) network: Vec<String>,
    pub(crate) storage_kib: u32,
    pub(crate) uses: [bool; 3],
    pub(crate) media: bool,
    pub(crate) own_volume: bool,
    pub(crate) fps: u32,
}

pub(crate) fn host(id: &str, generation: u64) -> String {
    let hash = wprs_format::sha256(format!("{id}#{generation}").as_bytes()).unwrap_or_default();
    format!("{}.{DOMAIN}", &hash[..16.min(hash.len())])
}

pub(crate) fn passing() -> String {
    let random = std::hash::BuildHasher::hash_one(&std::hash::RandomState::new(), std::time::Instant::now());
    format!("{random:016x}.e.{DOMAIN}")
}

fn web(package: &mut Package<BufReader<File>>, path: &Path) -> Result<Web, Error> {
    let m = &package.manifest;
    let (own_volume, fps) =
        (m.properties.contains_key(VOLUME), m.playback.clone().unwrap_or_default().frame_cap());
    let block = m.web.clone().unwrap_or_default();
    let uses = [Input::Audio, Input::Pointer, Input::Clicks].map(|i| m.inputs.contains(&i));
    let media = m.inputs.contains(&Input::Media);
    let network = m.permissions.iter().flat_map(|p| &p.network).flat_map(|n| n.allow.clone()).collect();
    let entries = package.entries.iter().map(|e| e.path.clone());
    let entries = entries.filter(|p| p != wprs_format::MANIFEST && p != wprs_format::SIGNATURE).collect();
    let entry = m.entry.clone();
    let preload = match block.preload {
        Some(file) => Some(
            String::from_utf8(package.read_file(&file)?)
                .map_err(|_| said("android.utf_text", "%1 is not UTF-8 text", &[&file]))?,
        ),
        None => None,
    };
    let background = block.background.unwrap_or_else(|| "#000000".into());
    let (wpak, storage_kib) = (path.to_owned(), block.storage_kib);
    Ok(Web { wpak, entry, entries, preload, background, network, storage_kib, uses, media, own_volume, fps })
}

pub(crate) fn is_web(package: &Package<BufReader<File>>) -> bool {
    package.manifest.kind == Kind::Web
}

pub(crate) fn pack(package: &mut Package<BufReader<File>>, path: &Path) -> Result<Pack, Error> {
    let web = web(package, path)?;
    let (m, inputs) = (package.manifest.localized(), package.manifest.declared());
    let mut properties = m.properties.clone();
    let loudness = m.playback.clone().unwrap_or_default().loudness();
    properties.entry(VOLUME.into()).or_insert_with(|| wprs_scene::volume_property("Volume", loudness));
    let labels = wprs_scene::labels(&properties, &m.default_locale.iter().cloned().collect::<Vec<_>>());
    Ok(Pack {
        id: m.id,
        version: m.version,
        locale: m.default_locale,
        scene: Arc::new(crate::pack::blank()?),
        properties,
        presets: m.presets,
        assets: Default::default(),
        videos: Vec::new(),
        inputs,
        scripts: Vec::new(),
        labels,
        web: Some(web),
        video: None,
    })
}
