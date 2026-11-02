use crate::rigged::doc::{Gltf, Reference};

pub const SLOTS: [&str; 5] = ["base", "orm", "normal", "occlusion", "emissive"];

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Alpha {
    #[default]
    Opaque,
    Mask(f32),
    Blend,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finish {
    pub name: String,
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub normal_scale: f32,
    pub occlusion: f32,
    pub alpha: Alpha,
    pub double_sided: bool,
    pub unlit: bool,
    pub maps: [Option<u16>; 5],
}

impl Default for Finish {
    fn default() -> Self {
        Finish {
            name: "default".into(),
            color: [1.0; 4],
            metallic: 1.0,
            roughness: 1.0,
            emissive: [0.0; 3],
            normal_scale: 1.0,
            occlusion: 1.0,
            alpha: Alpha::Opaque,
            double_sided: false,
            unlit: false,
            maps: [None; 5],
        }
    }
}

fn finite(v: &[f32]) -> bool {
    v.iter().all(|x| x.is_finite())
}

pub(super) fn finishes(doc: &Gltf, textures: usize) -> Result<Vec<Finish>, String> {
    let mut out = Vec::with_capacity(doc.materials.len());
    for (i, m) in doc.materials.iter().enumerate() {
        let name = m.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("material_{i}"));
        let slot = |r: &Option<Reference>| -> Result<Option<u16>, String> {
            match r {
                Some(r) if r.index >= textures => {
                    Err(format!("material {name} names the missing texture {}", r.index))
                }
                Some(r) => Ok(Some(r.index as u16)),
                None => Ok(None),
            }
        };
        let pbr = m.pbr.as_ref();
        let strength = m.extensions.get("KHR_materials_emissive_strength");
        let strength = strength.and_then(|e| e["emissiveStrength"].as_f64()).unwrap_or(1.0) as f32;
        let emissive = m.emissive.unwrap_or([0.0; 3]).map(|v| v * strength);
        let alpha = match m.alpha.as_deref() {
            Some("MASK") => Alpha::Mask(m.cutoff.unwrap_or(0.5)),
            Some("BLEND") => Alpha::Blend,
            _ => Alpha::Opaque,
        };
        let finish = Finish {
            color: pbr.and_then(|p| p.color).unwrap_or([1.0; 4]),
            metallic: pbr.and_then(|p| p.metallic).unwrap_or(1.0),
            roughness: pbr.and_then(|p| p.roughness).unwrap_or(1.0),
            emissive,
            normal_scale: m.normal.as_ref().and_then(|r| r.scale).unwrap_or(1.0),
            occlusion: m.occlusion.as_ref().and_then(|r| r.strength).unwrap_or(1.0),
            alpha,
            double_sided: m.double,
            unlit: m.extensions.contains_key("KHR_materials_unlit"),
            maps: [
                slot(&pbr.and_then(|p| p.texture.as_ref().map(copy)))?,
                slot(&pbr.and_then(|p| p.orm.as_ref().map(copy)))?,
                slot(&m.normal.as_ref().map(copy))?,
                slot(&m.occlusion.as_ref().map(copy))?,
                slot(&m.glow.as_ref().map(copy))?,
            ],
            name,
        };
        let numbers = [finish.metallic, finish.roughness, finish.normal_scale, finish.occlusion];
        if !finite(&finish.color) || !finite(&finish.emissive) || !finite(&numbers) {
            return Err(format!("material {} holds a value that is not a number", finish.name));
        }
        out.push(finish);
    }
    Ok(out)
}

fn copy(r: &Reference) -> Reference {
    Reference { index: r.index, set: r.set, scale: r.scale, strength: r.strength }
}
