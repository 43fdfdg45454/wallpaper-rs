use super::{ForgeMaterial, Out, shapes};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct ForgeRig {
    pub height: f32,
    pub width: f32,
    pub rows: u32,
    pub material: ForgeMaterial,
}

impl ForgeRig {
    pub fn new(height: f32, width: f32) -> ForgeRig {
        ForgeRig { height, width, rows: 8, material: ForgeMaterial::new("body", [1.0; 4]) }
    }

    fn column(&self) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<u32>) {
        let (h, w, rows) = (self.height, self.width * 0.5, self.rows.max(1));
        let corners = [[-w, w], [w, w], [w, -w], [-w, -w]];
        let (mut p, mut n, mut uv, mut list) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for r in 0..=rows {
            let y = h * r as f32 / rows as f32;
            for (c, [x, z]) in corners.iter().enumerate() {
                p.extend([*x, y, *z]);
                let l = (x * x + z * z).sqrt().max(1e-6);
                n.extend([x / l, 0.0, z / l]);
                uv.extend([c as f32 / 4.0, 1.0 - r as f32 / rows as f32]);
            }
        }
        for r in 0..rows {
            for s in 0..4u32 {
                let (a, b) = (r * 4 + s, r * 4 + (s + 1) % 4);
                list.extend([a, b, b + 4, a, b + 4, a + 4]);
            }
        }
        (p, n, uv, list)
    }

    pub fn glb(&self) -> Vec<u8> {
        let mut out = Out { bin: Vec::new(), views: Vec::new(), accessors: Vec::new() };
        let (p, n, uv, list) = self.column();
        let count = p.len() / 3;
        let h = self.height;
        let tip = |y: f32| ((y - 0.25 * h) / (0.5 * h)).clamp(0.0, 1.0);
        let weights: Vec<f32> =
            (0..count).flat_map(|v| [1.0 - tip(p[v * 3 + 1]), tip(p[v * 3 + 1]), 0.0, 0.0]).collect();
        let joints: Vec<u16> = (0..count).flat_map(|_| [0u16, 1, 0, 0]).collect();
        let colors: Vec<f32> = (0..count).flat_map(|v| [p[v * 3 + 1] / h, 0.0, 0.0, 1.0]).collect();
        let bulge: Vec<f32> = (0..count)
            .flat_map(|v| {
                let k = 0.4 * (1.0 - (p[v * 3 + 1] / h - 0.5).abs() * 2.0);
                [p[v * 3] * k, 0.0, p[v * 3 + 2] * k]
            })
            .collect();
        let view = out.view(bytemuck::cast_slice(&joints));
        out.accessors
            .push(json!({ "bufferView": view, "componentType": 5123, "count": count, "type": "VEC4" }));
        let joint_at = out.accessors.len() - 1;
        let attributes = json!({ "POSITION": out.floats(&p, 3, "VEC3"), "NORMAL": out.floats(&n, 3, "VEC3"),
            "TEXCOORD_0": out.floats(&uv, 2, "VEC2"), "JOINTS_0": joint_at,
            "WEIGHTS_0": out.floats(&weights, 4, "VEC4"), "COLOR_0": out.floats(&colors, 4, "VEC4") });
        let target = json!({ "POSITION": out.floats(&bulge, 3, "VEC3") });
        let primitive = json!({ "attributes": attributes, "indices": out.indices(&list), "material": 0, "targets": [target] });
        let mut inverse = vec![0.0f32; 32];
        [0, 5, 10, 15, 16, 21, 26, 31].iter().for_each(|i| inverse[*i] = 1.0);
        inverse[29] = -h * 0.5;
        let ibm = out.floats(&inverse, 16, "MAT4");
        let (mut textures, mut images) = (Vec::new(), Vec::new());
        let material = shapes::material(&self.material, &mut out, &mut textures, &mut images);
        let s45 = (std::f32::consts::FRAC_PI_8).sin();
        let c45 = (std::f32::consts::FRAC_PI_8).cos();
        let times = out.floats(&[0.0, 1.0, 2.0], 1, "SCALAR");
        let turns = out.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, s45, c45, 0.0, 0.0, 0.0, 1.0], 4, "VEC4");
        let walk_t = out.floats(&[0.0, 1.0], 1, "SCALAR");
        let walk = out.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0], 3, "VEC3");
        let animations = json!([
            { "name": "bend", "samplers": [{ "input": times, "output": turns }],
              "channels": [{ "sampler": 0, "target": { "node": 1, "path": "rotation" } }] },
            { "name": "walk", "samplers": [{ "input": walk_t, "output": walk }],
              "channels": [{ "sampler": 0, "target": { "node": 0, "path": "translation" } }] }
        ]);
        let nodes = json!([
            { "name": "root", "children": [1] },
            { "name": "tip", "translation": [0.0, h * 0.5, 0.0] },
            { "name": "body", "mesh": 0, "skin": 0 }
        ]);
        let doc = json!({ "asset": { "version": "2.0" }, "scene": 0, "scenes": [{ "nodes": [0, 2] }],
            "nodes": nodes, "skins": [{ "joints": [0, 1], "inverseBindMatrices": ibm }],
            "meshes": [{ "name": "body", "primitives": [primitive], "weights": [0.0], "extras": { "targetNames": ["bulge"] } }],
            "materials": [material], "textures": textures, "images": images, "animations": animations,
            "accessors": out.accessors, "bufferViews": out.views });
        super::pack(doc, out.bin)
    }
}

pub(super) fn finish(mut doc: Value, mut bin: Vec<u8>) -> Vec<u8> {
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    doc["buffers"] = json!([{ "byteLength": bin.len() }]);
    let mut text = doc.to_string().into_bytes();
    while !text.len().is_multiple_of(4) {
        text.push(b' ');
    }
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total);
    for word in [0x4654_6C67, 2, total as u32, text.len() as u32, 0x4E4F_534A] {
        glb.extend_from_slice(&u32::to_le_bytes(word));
    }
    glb.extend_from_slice(&text);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    glb.extend_from_slice(&bin);
    glb
}
