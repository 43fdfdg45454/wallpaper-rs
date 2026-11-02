use super::json::Doc;
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Surface {
    pub name: Option<String>,
    #[serde(rename = "pbrMetallicRoughness")]
    pub pbr: Option<Pbr>,
    #[serde(rename = "normalTexture")]
    pub normal: Option<Slot>,
    #[serde(rename = "occlusionTexture")]
    pub occlusion: Option<Slot>,
    #[serde(rename = "emissiveTexture")]
    pub glow: Option<Slot>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Pbr {
    #[serde(rename = "baseColorTexture")]
    pub base: Option<Slot>,
    #[serde(rename = "metallicRoughnessTexture")]
    pub orm: Option<Slot>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Slot {
    #[serde(rename = "texCoord")]
    pub set: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Sampler {
    #[serde(rename = "wrapS")]
    pub s: Option<u32>,
    #[serde(rename = "wrapT")]
    pub t: Option<u32>,
}

pub(super) fn notes(doc: &Doc) -> Vec<String> {
    let mut out = Vec::new();
    if doc.used.iter().any(|e| e == "KHR_texture_transform") {
        out.push("moves its textures with KHR_texture_transform, which is drawn without it".to_owned());
    }
    let slots = |m: &Surface| {
        let pbr = m.pbr.as_ref();
        [
            pbr.and_then(|p| p.base.as_ref()),
            pbr.and_then(|p| p.orm.as_ref()),
            m.normal.as_ref(),
            m.occlusion.as_ref(),
            m.glow.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|s| s.set != 0)
    };
    if let Some(m) = doc.materials.iter().find(|m| slots(m)) {
        let name = m.name.clone().unwrap_or_else(|| "a material".to_owned());
        out.push(format!("{name} reads a second set of texture coordinates, drawn with the first"));
    }
    if doc.samplers.iter().any(|s| s.s.is_some_and(|w| w != 10497) || s.t.is_some_and(|w| w != 10497)) {
        out.push("clamps or mirrors a texture, which is drawn repeated".to_owned());
    }
    out
}
