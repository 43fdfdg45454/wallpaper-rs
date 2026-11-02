use crate::{Error, Result, Scene, check_path, require};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_PARTS: usize = 256;
pub const PART_KINDS: [&str; 4] = ["layer", "particles", "script", "effect"];
const THUMBNAIL_BYTES: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub kind: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

fn offered(scene: &Scene) -> Vec<Part> {
    let part = |kind: &str, id: &str| Part { kind: kind.into(), id: id.into(), title: None, thumbnail: None };
    let layers = scene.layers.iter().map(|l| part("layer", &l.id));
    let particles = scene.particles.keys().map(|k| part("particles", k));
    let scripts = scene.scripts.iter().map(|s| part("script", &s.path));
    layers.chain(particles).chain(scripts).chain(scene.effects.keys().map(|k| part("effect", k))).collect()
}

pub fn exists(scene: &Scene, kind: &str, id: &str) -> bool {
    match kind {
        "layer" => scene.layers.iter().any(|l| l.id == id),
        "particles" => scene.particles.contains_key(id),
        "script" => scene.scripts.iter().any(|s| s.path == id),
        "effect" => scene.effects.contains_key(id),
        _ => false,
    }
}

pub fn parts(scene: &Scene) -> Result<Vec<Part>> {
    let Some(value) = scene.ext.get("parts") else { return Ok(offered(scene)) };
    let field = "ext.parts";
    let list: Vec<Part> = serde_json::from_value(value.clone())
        .map_err(|e| Error::Invalid { field: field.into(), reason: e.to_string().into() })?;
    require(list.len() <= MAX_PARTS, field, "lists at most 256 parts")?;
    let mut seen = HashSet::new();
    for p in &list {
        require(PART_KINDS.contains(&p.kind.as_str()), field, "kind is layer, particles, script or effect")?;
        require(exists(scene, &p.kind, &p.id), field, "names a part that the document does not hold")?;
        require(seen.insert((&p.kind, &p.id)), field, "names a part twice")?;
        let title = p.title.as_deref().unwrap_or("x");
        let plain = !title.contains(['<', '>', '\n', '\r']);
        require(
            (1..=128).contains(&title.chars().count()) && plain,
            field,
            "title has 1 to 128 characters on one line",
        )?;
        if let Some(path) = &p.thumbnail {
            check_path(path)?;
        }
    }
    Ok(list)
}

pub(crate) fn check(
    scene: &Scene,
    size: impl Fn(&str) -> Option<u64>,
    mut read: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<()> {
    for path in parts(scene)?.iter().filter_map(|p| p.thumbnail.as_deref()) {
        let bytes = size(path).ok_or_else(|| Error::Missing(path.into()))?;
        require(bytes <= THUMBNAIL_BYTES, "ext.parts", "a thumbnail has at most 1 MiB")?;
        crate::media::preview(false, path, &read(path)?)?;
    }
    Ok(())
}
