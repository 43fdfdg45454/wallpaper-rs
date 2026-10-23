use crate::build::collect;
use crate::features::{Facts, facts};
use crate::{Error, Kind, Limits, MANIFEST, Manifest, Result, SIGNATURE, Scene, media, require, validate};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

pub fn inspect(source: impl AsRef<Path>) -> Result<(Manifest, Facts)> {
    let source = source.as_ref();
    let raw = fs::read(source.join(MANIFEST))?;
    let manifest = Manifest::from_json(&raw)?;
    let facts = facts_of(source, &manifest, &raw, None)?;
    Ok((manifest, facts))
}

pub(crate) fn facts_of(
    source: &Path,
    manifest: &Manifest,
    raw: &[u8],
    scene: Option<String>,
) -> Result<Facts> {
    let mut files = Vec::new();
    collect(source, "", &mut files)?;
    validate(manifest, &files, &Limits::default())?;
    let entry = source.join(&manifest.entry);
    let scene = match (manifest.kind, scene) {
        (Kind::Scene, Some(text)) => Some(Scene::load(&text, &manifest.properties)),
        (Kind::Scene, None) => Some(Scene::load(&fs::read_to_string(&entry)?, &manifest.properties)),
        _ => None,
    };
    let scene =
        scene.transpose().map_err(|e| Error::Invalid { field: "scene".into(), reason: e.to_string() })?;
    let mut head = Vec::new();
    if manifest.kind == Kind::Image {
        File::open(&entry)?.take(1 << 16).read_to_end(&mut head)?;
    }
    let size = media::probe(&head).map(|m| [m.width, m.height]);
    let mut found = facts(manifest, scene.as_ref(), size);
    found.features.extend(Manifest::newer_spelling(raw).map(|_| "properties2".to_owned()));
    found.notes = scene.iter().flat_map(|s| uneven(source, s)).collect();
    Ok(found)
}

fn uneven(source: &Path, scene: &Scene) -> Vec<String> {
    let ratio = |id: &str| {
        let mut head = Vec::new();
        File::open(source.join(&scene.assets.get(id)?.path))
            .ok()?
            .take(1 << 16)
            .read_to_end(&mut head)
            .ok()?;
        media::probe(&head).map(|m| f64::from(m.width) / f64::from(m.height.max(1)))
    };
    let varied = |(place, ids): (String, Vec<&str>)| {
        let ratios: Vec<f64> = ids.into_iter().filter_map(ratio).collect();
        let (low, high) = ratios.iter().fold((f64::MAX, 0.0f64), |(a, b), r| (a.min(*r), b.max(*r)));
        (ratios.len() > 1 && high > low * 1.01)
            .then(|| format!("the images of {place} do not share one proportion, so some will be cropped"))
    };
    wprs_scene::variants(scene).into_iter().filter_map(varied).collect()
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
