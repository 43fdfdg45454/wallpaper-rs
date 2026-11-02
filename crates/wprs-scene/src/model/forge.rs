use serde_json::{Value, json};

mod rig;
mod shapes;

pub use rig::ForgeRig;
use rig::finish as pack;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForgeShape {
    Cube,
    Sphere(u32),
    Plane,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForgeMaterial {
    pub name: String,
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub maps: [Option<Vec<u8>>; 5],
    pub alpha: Option<&'static str>,
    pub double_sided: bool,
    pub unlit: bool,
}

impl ForgeMaterial {
    pub fn new(name: &str, color: [f32; 4]) -> ForgeMaterial {
        let (emissive, maps) = ([0.0; 3], Default::default());
        let (alpha, double_sided, unlit) = (None, false, false);
        ForgeMaterial {
            name: name.into(),
            color,
            metallic: 0.0,
            roughness: 0.5,
            emissive,
            maps,
            alpha,
            double_sided,
            unlit,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Forge {
    pub parts: Vec<(String, ForgeShape, [f32; 3], f32, usize)>,
    pub materials: Vec<ForgeMaterial>,
}

struct Out {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Out {
    fn view(&mut self, bytes: &[u8]) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let at = self.bin.len();
        self.bin.extend_from_slice(bytes);
        self.views.push(json!({ "buffer": 0, "byteOffset": at, "byteLength": bytes.len() }));
        self.views.len() - 1
    }

    fn floats(&mut self, data: &[f32], width: usize, kind: &str) -> usize {
        let view = self.view(bytemuck::cast_slice(data));
        let mut a =
            json!({ "bufferView": view, "componentType": 5126, "count": data.len() / width, "type": kind });
        if width == 3 || width == 1 {
            let range = |f: fn(f32, f32) -> f32, s: f32| {
                (0..width).map(|k| data.iter().skip(k).step_by(width).copied().fold(s, f)).collect::<Vec<_>>()
            };
            (a["min"], a["max"]) = (json!(range(f32::min, f32::MAX)), json!(range(f32::max, f32::MIN)));
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }

    fn indices(&mut self, data: &[u32]) -> usize {
        let view = self.view(bytemuck::cast_slice(data));
        self.accessors.push(
            json!({ "bufferView": view, "componentType": 5125, "count": data.len(), "type": "SCALAR" }),
        );
        self.accessors.len() - 1
    }
}

impl Forge {
    pub fn material(mut self, m: ForgeMaterial) -> Forge {
        self.materials.push(m);
        self
    }

    pub fn part(mut self, name: &str, shape: ForgeShape, at: [f32; 3], size: f32, material: usize) -> Forge {
        self.parts.push((name.into(), shape, at, size, material));
        self
    }

    pub fn glb(&self) -> Vec<u8> {
        let mut out = Out { bin: Vec::new(), views: Vec::new(), accessors: Vec::new() };
        let (mut nodes, mut meshes) = (Vec::new(), Vec::new());
        for (i, (name, shape, at, size, material)) in self.parts.iter().enumerate() {
            let (p, n, uv, list) = shapes::shape(*shape, *size);
            let attributes = json!({ "POSITION": out.floats(&p, 3, "VEC3"), "NORMAL": out.floats(&n, 3, "VEC3"),
                "TEXCOORD_0": out.floats(&uv, 2, "VEC2") });
            let primitive =
                json!({ "attributes": attributes, "indices": out.indices(&list), "material": material });
            meshes.push(json!({ "name": name, "primitives": [primitive] }));
            nodes.push(json!({ "name": name, "mesh": i, "translation": at }));
        }
        let (mut textures, mut images) = (Vec::new(), Vec::new());
        let materials: Vec<Value> = self
            .materials
            .iter()
            .map(|m| shapes::material(m, &mut out, &mut textures, &mut images))
            .collect();
        let doc = json!({ "asset": { "version": "2.0" }, "scene": 0, "scenes": [{ "nodes": (0..nodes.len()).collect::<Vec<_>>() }],
            "nodes": nodes, "meshes": meshes, "materials": materials, "textures": textures, "images": images,
            "accessors": out.accessors, "bufferViews": out.views });
        pack(doc, out.bin)
    }
}
