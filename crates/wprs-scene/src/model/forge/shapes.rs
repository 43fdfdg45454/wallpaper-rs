use super::{ForgeMaterial, ForgeShape, Out};
use serde_json::{Value, json};
use std::f32::consts::PI;

type Built = (Vec<f32>, Vec<f32>, Vec<f32>, Vec<u32>);

fn cube(h: f32) -> Built {
    let (mut p, mut n, mut uv, mut list) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    for (f, (normal, u, v)) in faces.iter().enumerate() {
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            p.extend((0..3).map(|k| (normal[k] + u[k] * a + v[k] * b) * h));
            n.extend_from_slice(normal);
            uv.extend([(a + 1.0) * 0.5, (1.0 - b) * 0.5]);
        }
        let base = f as u32 * 4;
        list.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (p, n, uv, list)
}

fn sphere(r: f32, detail: u32) -> Built {
    let (rings, segments) = (detail.max(3), detail.max(3) * 2);
    let (mut p, mut n, mut uv, mut list) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for i in 0..=rings {
        let t = i as f32 / rings as f32 * PI;
        for j in 0..=segments {
            let s = j as f32 / segments as f32 * 2.0 * PI;
            let d = [t.sin() * s.sin(), t.cos(), t.sin() * s.cos()];
            p.extend(d.map(|v| v * r));
            n.extend(d);
            uv.extend([j as f32 / segments as f32, i as f32 / rings as f32]);
        }
    }
    let row = segments + 1;
    for i in 0..rings {
        for j in 0..segments {
            let (a, b) = (i * row + j, (i + 1) * row + j);
            list.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    (p, n, uv, list)
}

pub(super) fn shape(shape: ForgeShape, size: f32) -> Built {
    let h = size * 0.5;
    match shape {
        ForgeShape::Cube => cube(h),
        ForgeShape::Sphere(detail) => sphere(h, detail),
        ForgeShape::Plane => {
            let p = vec![-h, -h, 0.0, h, -h, 0.0, h, h, 0.0, -h, h, 0.0];
            let n = [0.0, 0.0, 1.0].repeat(4);
            (p, n, vec![0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0], vec![0, 1, 2, 0, 2, 3])
        }
    }
}

pub(super) fn material(
    m: &ForgeMaterial,
    out: &mut Out,
    textures: &mut Vec<Value>,
    images: &mut Vec<Value>,
) -> Value {
    let mut slot = |bytes: &Option<Vec<u8>>| {
        let bytes = bytes.as_ref()?;
        let view = out.view(bytes);
        let mime = if bytes.starts_with(&[0xFF, 0xD8]) { "image/jpeg" } else { "image/png" };
        images.push(json!({ "bufferView": view, "mimeType": mime }));
        textures.push(json!({ "source": images.len() - 1 }));
        Some(json!({ "index": textures.len() - 1 }))
    };
    let [base, orm, normal, occlusion, glow] = [0, 1, 2, 3, 4].map(|k| slot(&m.maps[k]));
    let mut pbr =
        json!({ "baseColorFactor": m.color, "metallicFactor": m.metallic, "roughnessFactor": m.roughness });
    let mut v = json!({ "name": m.name, "emissiveFactor": m.emissive, "doubleSided": m.double_sided });
    [("baseColorTexture", base), ("metallicRoughnessTexture", orm)]
        .into_iter()
        .for_each(|(k, t)| t.into_iter().for_each(|t| pbr[k] = t));
    let more = [("normalTexture", normal), ("occlusionTexture", occlusion), ("emissiveTexture", glow)];
    more.into_iter().for_each(|(k, t)| t.into_iter().for_each(|t| v[k] = t));
    v["pbrMetallicRoughness"] = pbr;
    if let Some(a) = m.alpha {
        v["alphaMode"] = json!(a);
    }
    if m.unlit {
        v["extensions"] = json!({ "KHR_materials_unlit": {} });
    }
    v
}
