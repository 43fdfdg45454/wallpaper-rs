use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

pub struct Gl {
    pub doc: Value,
    pub bin: Vec<u8>,
}

impl Gl {
    pub fn new() -> Gl {
        let doc = json!({ "asset": { "version": "2.0" }, "nodes": [], "meshes": [], "accessors": [], "bufferViews": [] });
        Gl { doc, bin: Vec::new() }
    }

    fn add(&mut self, bytes: &[u8], component: u32, kind: &str, count: usize) -> usize {
        let offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        self.bin.resize(self.bin.len().div_ceil(4) * 4, 0);
        let views = self.doc["bufferViews"].as_array_mut().unwrap();
        views.push(json!({ "buffer": 0, "byteOffset": offset, "byteLength": bytes.len() }));
        let view = views.len() - 1;
        let accessors = self.doc["accessors"].as_array_mut().unwrap();
        accessors
            .push(json!({ "bufferView": view, "componentType": component, "type": kind, "count": count }));
        accessors.len() - 1
    }

    pub fn floats(&mut self, kind: &str, values: &[f32]) -> usize {
        let width = match kind {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            _ => 16,
        };
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.add(&bytes, 5126, kind, values.len() / width)
    }

    pub fn joints(&mut self, values: &[u16]) -> usize {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.add(&bytes, 5123, "VEC4", values.len() / 4)
    }

    pub fn indices(&mut self, values: &[u32]) -> usize {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.add(&bytes, 5125, "SCALAR", values.len())
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut doc = self.doc.clone();
        doc["buffers"] = json!([{ "byteLength": self.bin.len() }]);
        let mut text = doc.to_string().into_bytes();
        text.resize(text.len().div_ceil(4) * 4, b' ');
        let total = 28 + text.len() + self.bin.len();
        let mut out = Vec::new();
        for w in [0x4654_6C67u32, 2, total as u32, text.len() as u32, 0x4E4F_534A] {
            out.extend(w.to_le_bytes());
        }
        out.extend(text);
        out.extend((self.bin.len() as u32).to_le_bytes());
        out.extend(0x004E_4942u32.to_le_bytes());
        out.extend(&self.bin);
        out
    }
}

pub const XS: [f32; 5] = [-24.0, -12.0, 0.0, 12.0, 24.0];

pub fn arm() -> Gl {
    let mut gl = Gl::new();
    let mut positions = Vec::new();
    let (mut joints, mut weights, mut uvs) = (Vec::new(), Vec::new(), Vec::new());
    for y in [-4.0f32, 4.0] {
        for x in XS {
            positions.extend([x, y, 0.0]);
            uvs.extend([(x + 32.0) / 64.0, (32.0 - y) / 64.0]);
            let (j, w) = match x {
                x if x < 0.0 => ([0u16, 0, 0, 0], [1.0f32, 0.0, 0.0, 0.0]),
                0.0 => ([0, 1, 0, 0], [0.5, 0.5, 0.0, 0.0]),
                _ => ([1, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
            };
            joints.extend(j);
            weights.extend(w);
        }
    }
    let quads = (0..4u32).flat_map(|i| [i, i + 1, i + 6, i, i + 6, i + 5]);
    let list: Vec<u32> = quads.collect();
    let position = gl.floats("VEC3", &positions);
    let uv = gl.floats("VEC2", &uvs);
    let j = gl.joints(&joints);
    let w = gl.floats("VEC4", &weights);
    let indices = gl.indices(&list);
    let lift = gl.floats("VEC3", &(0..10).flat_map(|_| [0.0, 2.0, 0.0]).collect::<Vec<_>>());
    let inverse = |x: f32| [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -x, 0.0, 0.0, 1.0];
    let bind = gl.floats("MAT4", &[inverse(-24.0), inverse(0.0), inverse(24.0)].concat());
    let patch = gl.floats("VEC3", &[20.0, -2.0, 0.0, 28.0, -2.0, 0.0, 28.0, 2.0, 0.0, 20.0, 2.0, 0.0]);
    let quad = gl.indices(&[0, 1, 2, 0, 2, 3]);
    let times = gl.floats("SCALAR", &[0.0, 1.0]);
    let (s, c) = (std::f32::consts::FRAC_PI_4.sin(), std::f32::consts::FRAC_PI_4.cos());
    let turns = gl.floats("VEC4", &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, s, c]);
    let lifts = gl.floats("SCALAR", &[0.0, 1.0]);
    gl.doc["nodes"] = json!([
        { "mesh": 0, "skin": 0 },
        { "name": "root", "translation": [-24.0, 0.0, 0.0], "children": [2] },
        { "name": "arm", "translation": [24.0, 0.0, 0.0], "children": [3] },
        { "name": "hand", "translation": [24.0, 0.0, 0.0] },
        { "name": "patch", "mesh": 1 }
    ]);
    gl.doc["skins"] = json!([{ "joints": [1, 2, 3], "inverseBindMatrices": bind }]);
    gl.doc["meshes"] = json!([
        { "name": "body", "extras": { "targetNames": ["lift"] }, "primitives": [{ "attributes": {
            "POSITION": position, "TEXCOORD_0": uv, "JOINTS_0": j, "WEIGHTS_0": w }, "indices": indices,
            "targets": [{ "POSITION": lift }] }] },
        { "name": "patch", "primitives": [{ "attributes": { "POSITION": patch }, "indices": quad }] }
    ]);
    gl.doc["animations"] = json!([{ "name": "wave",
        "samplers": [{ "input": times, "output": turns }, { "input": times, "output": lifts }],
        "channels": [{ "sampler": 0, "target": { "node": 2, "path": "rotation" } },
                     { "sampler": 1, "target": { "node": 0, "path": "weights" } }] }]);
    gl
}

pub fn scene(layer: Value, glb: Vec<u8>) -> Result<Scene, SceneError> {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] },
        "assets": { "arm": { "path": "arm.glb" } }, "layers": [layer] });
    let mut scene = Scene::from_json(&text.to_string())?;
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone()))?;
    Ok(scene)
}

pub fn fox(extra: Value) -> Value {
    let mut layer = json!({ "id": "fox", "type": "mesh", "mesh": "arm", "size": [64, 64] });
    extra.as_object().unwrap().iter().for_each(|(k, v)| layer[k] = v.clone());
    layer
}

pub fn runtime(extra: Value) -> Runtime {
    Runtime::new(Arc::new(scene(fox(extra), arm().bytes()).unwrap()), Values::new())
}

pub fn moved(rt: &Runtime, bone: usize, p: [f32; 3]) -> [f32; 3] {
    let m = rt.puppet("fox").unwrap().posed().palette[bone];
    std::array::from_fn(|r| m[r * 4] * p[0] + m[r * 4 + 1] * p[1] + m[r * 4 + 2] * p[2] + m[r * 4 + 3])
}

pub fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

pub fn at(rt: &Runtime, bone: &str) -> [f32; 2] {
    let p = rt.puppet("fox").unwrap();
    let f = p.bone_frames([64.0, 64.0])[p.rig.joint(bone).unwrap()];
    [f[4] - 32.0, 32.0 - f[5]]
}

pub fn close(a: [f32; 2], b: [f32; 2], within: f32) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < within
}

pub fn layers(list: Value) -> Runtime {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] },
        "assets": { "arm": { "path": "arm.glb" } }, "layers": list });
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let glb = arm().bytes();
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    Runtime::new(Arc::new(scene), Values::new())
}
