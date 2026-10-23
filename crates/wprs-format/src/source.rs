use crate::build::collect;
use crate::features::{Facts, facts};
use crate::meta::{Content, Origin, Previews};
use crate::{
    Error, Kind, Limits, MANIFEST, Manifest, Result, SCHEMA, SIGNATURE, Scene, media, require, validate,
};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

pub fn inspect(source: impl AsRef<Path>) -> Result<(Manifest, Facts)> {
    let source = source.as_ref();
    let manifest = Manifest::from_json(&fs::read(source.join(MANIFEST))?)?;
    let facts = facts_of(source, &manifest)?;
    Ok((manifest, facts))
}

fn facts_of(source: &Path, manifest: &Manifest) -> Result<Facts> {
    let mut files = Vec::new();
    collect(source, "", &mut files)?;
    validate(manifest, &files, &Limits::default())?;
    let entry = source.join(&manifest.entry);
    let scene = match manifest.kind {
        Kind::Scene => Some(Scene::load(&fs::read_to_string(&entry)?, &manifest.properties)),
        _ => None,
    };
    let scene =
        scene.transpose().map_err(|e| Error::Invalid { field: "scene".into(), reason: e.to_string() })?;
    let mut head = Vec::new();
    if manifest.kind == Kind::Image {
        File::open(&entry)?.take(1 << 16).read_to_end(&mut head)?;
    }
    let size = media::probe(&head).map(|m| [m.width, m.height]);
    Ok(facts(manifest, scene.as_ref(), size))
}

pub fn fill(source: impl AsRef<Path>) -> Result<Vec<String>> {
    let source = source.as_ref();
    let (mut manifest, facts) = inspect(source)?;
    let listed = manifest.features();
    let added: Vec<String> = facts.features.into_iter().filter(|f| !listed.contains(f)).collect();
    if !manifest.v2() || added.is_empty() {
        return Ok(Vec::new());
    }
    let reason = format!("misses {added:?}; run pack sign again, which fills the list before signing");
    require(!source.join(SIGNATURE).exists(), "requires.features", &reason)?;
    let features = &mut manifest.requires.get_or_insert_default().features;
    features.extend(added.iter().cloned());
    features.sort();
    fs::write(source.join(MANIFEST), manifest.to_json()? + "\n")?;
    Ok(added)
}

pub fn upgrade(
    source: impl AsRef<Path>,
    content: Content,
    photosensitive: bool,
    origin: Origin,
) -> Result<Manifest> {
    let source = source.as_ref();
    let mut manifest = Manifest::from_json(&fs::read(source.join(MANIFEST))?)?;
    require(!manifest.v2(), "schema", "is already wallpaper-rs.wpak/2")?;
    let thumbnail = manifest.preview.take();
    manifest.previews = thumbnail.map(|t| Previews { thumbnail: Some(t), animated: None });
    (manifest.schema, manifest.content, manifest.photosensitive) =
        (SCHEMA.into(), Some(content), Some(photosensitive));
    manifest.origin = Some(origin);
    let facts = facts_of(source, &manifest)?;
    manifest.inputs = facts.inputs.into_iter().collect();
    let features = facts.features.into_iter().collect::<Vec<_>>();
    manifest.requires = (!features.is_empty()).then_some(crate::Requires { engine: None, features });
    fs::write(source.join(MANIFEST), manifest.to_json()? + "\n")?;
    let _ = fs::remove_file(source.join(SIGNATURE));
    Ok(manifest)
}
