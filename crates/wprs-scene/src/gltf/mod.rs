mod json;

use json::{Doc, within};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_MODEL_BYTES: usize = 64 << 20;
pub const MAX_MODEL_JSON: usize = 4 << 20;
pub const MAX_MODELS_JSON: usize = 16 << 20;
pub const MAX_MODEL_ENTRIES: usize = 1 << 18;
pub const MAX_NODES: usize = 1 << 16;
pub const MAX_MODEL_PARTS: usize = 4096;
pub const MAX_BONES: usize = 256;
pub const MAX_TRIANGLES: u64 = 2_000_000;
pub const MAX_MORPHS: usize = 64;
pub const EXTENSIONS: [&str; 4] = [
    "KHR_materials_emissive_strength",
    "KHR_materials_unlit",
    "KHR_mesh_quantization",
    "KHR_texture_transform",
];
const CROWDED: &str = "lists more entries than the models of a scene may hold";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelFile {
    pub nodes: BTreeSet<String>,
    pub bones: BTreeSet<String>,
    pub animations: BTreeMap<String, f32>,
    pub shapes: BTreeSet<String>,
    pub parts: BTreeSet<String>,
    pub materials: BTreeSet<String>,
    pub triangles: u64,
    pub skinned: bool,
}

fn name(named: &Option<String>, kind: &str, i: usize) -> String {
    named.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("{kind}_{i}"))
}

impl ModelFile {
    pub fn parse(bytes: &[u8]) -> Result<ModelFile, String> {
        let mut left = MAX_MODEL_ENTRIES;
        ModelFile::parse_head(bytes, bytes.len() as u64, &mut left)
    }

    pub fn json_len(head: &[u8]) -> Result<usize, String> {
        let word = |at: usize| head.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let (Some(b"glTF"), Some(2), Some(size), Some(0x4E4F_534A)) =
            (head.get(..4), word(4), word(12), word(16))
        else {
            return Err("is not a binary glTF 2.0 file".into());
        };
        match size as usize {
            size if size > MAX_MODEL_JSON => {
                Err(format!("has more than {} MiB of JSON", MAX_MODEL_JSON >> 20))
            }
            size => Ok(size),
        }
    }

    pub fn parse_head(head: &[u8], size: u64, left: &mut usize) -> Result<ModelFile, String> {
        let json = ModelFile::json_len(head)?;
        if head.get(8..12).map(|b| u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))) != Some(size) {
            return Err("has a length that does not match its header".into());
        }
        let json = head.get(20..20 + json).ok_or("has a cut JSON chunk")?;
        let doc = within(left, || serde_json::from_slice::<Doc>(json));
        let doc = doc.map_err(|e| match e.to_string() {
            text if text.starts_with(CROWDED) => CROWDED.to_owned(),
            text => format!("has invalid JSON: {text}"),
        })?;
        if !doc.asset.version.starts_with("2.") {
            return Err(format!("declares glTF {}, expected 2.0", doc.asset.version));
        }
        if doc.buffers.iter().chain(&doc.images).any(|l| l.uri.is_some()) {
            return Err("loads files from outside itself".into());
        }
        if let Some(ext) = doc.required.iter().find(|e| !EXTENSIONS.contains(&e.as_str())) {
            return Err(format!("requires the unsupported extension {ext}"));
        }
        let counts = [
            ("nodes", doc.nodes.len(), MAX_NODES),
            ("meshes", doc.meshes.len(), MAX_MODEL_PARTS),
            ("materials", doc.materials.len(), MAX_MODEL_PARTS),
            ("animations", doc.animations.len(), MAX_MODEL_PARTS),
            ("skins", doc.skins.len(), MAX_MODEL_PARTS),
        ];
        if let Some((what, count, max)) = counts.into_iter().find(|(_, count, max)| count > max) {
            return Err(format!("has {count} {what}, at most {max} are allowed"));
        }
        doc.summary()
    }
}

impl Doc {
    fn summary(&self) -> Result<ModelFile, String> {
        let nodes: Vec<String> =
            self.nodes.iter().enumerate().map(|(i, n)| name(&n.name, "node", i)).collect();
        let mut file = ModelFile::default();
        for skin in &self.skins {
            if skin.joints.len() > MAX_BONES {
                return Err(format!(
                    "has a skin of {} bones, at most {MAX_BONES} are allowed",
                    skin.joints.len()
                ));
            }
            file.bones.extend(skin.joints.iter().filter_map(|j| nodes.get(*j).cloned()));
        }
        file.nodes = nodes.into_iter().collect();
        let count = |i: Option<usize>| i.and_then(|i| self.accessors.get(i)).map_or(0, |a| a.count);
        for (i, mesh) in self.meshes.iter().enumerate() {
            file.parts.insert(name(&mesh.name, "mesh", i));
            let names = mesh.extras.as_ref().map_or(&[][..], |e| &e.names);
            for p in &mesh.primitives {
                let n = count(p.indices).max(count(p.attributes.position));
                file.triangles = file.triangles.saturating_add(match p.mode.unwrap_or(4) {
                    4 => n / 3,
                    5 | 6 => n.saturating_sub(2),
                    _ => 0,
                });
                let joints = p.attributes.joints.is_some() && p.attributes.weights.is_some();
                file.skinned |= joints && !self.skins.is_empty();
                if p.targets.len() > MAX_MORPHS {
                    return Err(format!(
                        "has {} blend shapes, at most {MAX_MORPHS} are allowed",
                        p.targets.len()
                    ));
                }
                let shapes =
                    (0..p.targets.len()).map(|t| names.get(t).map_or(format!("shape_{t}"), String::clone));
                file.shapes.extend(shapes);
            }
        }
        if file.triangles > MAX_TRIANGLES {
            return Err(format!("has {} triangles, at most {MAX_TRIANGLES} are allowed", file.triangles));
        }
        for (i, a) in self.animations.iter().enumerate() {
            let ends = a.samplers.iter().filter_map(|s| self.accessors.get(s.input)?.max.first().copied());
            file.animations.insert(name(&a.name, "animation", i), ends.fold(0.0, f64::max) as f32);
        }
        file.materials =
            self.materials.iter().enumerate().map(|(i, m)| name(&m.name, "material", i)).collect();
        Ok(file)
    }
}
