use super::sketch::Sketch;
use serde_json::{Value, json};

struct Out {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Out {
    fn add(&mut self, bytes: &[u8], accessor: Value) -> usize {
        let offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        self.bin.resize(self.bin.len().div_ceil(4) * 4, 0);
        self.views.push(json!({ "buffer": 0, "byteOffset": offset, "byteLength": bytes.len() }));
        let mut accessor = accessor;
        accessor["bufferView"] = json!(self.views.len() - 1);
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }
}

fn floats(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

impl Sketch {
    pub fn glb(&self) -> Vec<u8> {
        let mut out = Out { bin: Vec::new(), views: Vec::new(), accessors: Vec::new() };
        let n = self.points.len();
        let positions: Vec<[f32; 3]> = self.points.iter().map(|p| self.mesh(*p)).collect();
        let (lo, hi) = positions.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
            (std::array::from_fn(|i| lo[i].min(p[i])), std::array::from_fn(|i| hi[i].max(p[i])))
        });
        let (lo, hi) = if n == 0 { ([0.0; 3], [0.0; 3]) } else { (lo, hi) };
        let at = json!({ "componentType": 5126, "count": n, "type": "VEC3", "min": lo, "max": hi });
        let position = out.add(&floats(positions.iter().flatten().copied()), at);
        let uv =
            self.points.iter().flat_map(|p| [p[0] / self.size[0].max(1.0), p[1] / self.size[1].max(1.0)]);
        let uv = out.add(&floats(uv), json!({ "componentType": 5126, "count": n, "type": "VEC2" }));
        let list: Vec<u8> = self.triangles.iter().flatten().flat_map(|i| i.to_le_bytes()).collect();
        let count = self.triangles.len() * 3;
        let indices = out.add(&list, json!({ "componentType": 5125, "count": count, "type": "SCALAR" }));
        let mut attributes = json!({ "POSITION": position, "TEXCOORD_0": uv });
        let targets: Vec<Value> = self
            .shapes
            .iter()
            .map(|(_, moves)| {
                let deltas = (0..n).flat_map(|k| moves.get(k).map_or([0.0; 3], |d| [d[0], -d[1], 0.0]));
                let vec3 = json!({ "componentType": 5126, "count": n, "type": "VEC3" });
                json!({ "POSITION": out.add(&floats(deltas), vec3) })
            })
            .collect();
        let names: Vec<&str> = self.shapes.iter().map(|(name, _)| name.as_str()).collect();
        let mut nodes = vec![json!({ "mesh": 0 })];
        if !self.bones.is_empty() {
            let weights: Vec<[(u16, f32); 4]> = match self.weights.len() == n {
                true => self.weights.clone(),
                false => vec![[(0, 1.0), (0, 0.0), (0, 0.0), (0, 0.0)]; n],
            };
            let joints: Vec<u8> = weights.iter().flatten().flat_map(|(j, _)| j.to_le_bytes()).collect();
            let vec4 = |kind: u32| json!({ "componentType": kind, "count": n, "type": "VEC4" });
            attributes["JOINTS_0"] = json!(out.add(&joints, vec4(5123)));
            attributes["WEIGHTS_0"] =
                json!(out.add(&floats(weights.iter().flatten().map(|w| w.1)), vec4(5126)));
            let heads: Vec<[f32; 3]> = self.bones.iter().map(|b| self.mesh(b.at)).collect();
            let inverse = heads.iter().flat_map(|h| {
                [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -h[0], -h[1], -h[2], 1.0]
            });
            let bound = json!({ "componentType": 5126, "count": heads.len(), "type": "MAT4" });
            let inverse = out.add(&floats(inverse), bound);
            for (b, bone) in self.bones.iter().enumerate() {
                let from = bone.parent.map_or([0.0; 3], |p| heads[p]);
                let local: Vec<f32> = (0..3).map(|i| heads[b][i] - from[i]).collect();
                let children: Vec<usize> = (0..self.bones.len())
                    .filter(|c| self.bones[*c].parent == Some(b))
                    .map(|c| c + 1)
                    .collect();
                let mut node = json!({ "name": bone.name, "translation": local });
                if !children.is_empty() {
                    node["children"] = json!(children);
                }
                nodes.push(node);
            }
            nodes[0]["skin"] = json!(0);
            let joints: Vec<usize> = (1..=self.bones.len()).collect();
            let skins = json!([{ "joints": joints, "inverseBindMatrices": inverse }]);
            return glb(&out, nodes, (attributes, indices, targets, names), Some(skins));
        }
        glb(&out, nodes, (attributes, indices, targets, names), None)
    }
}

type Primitive<'a> = (Value, usize, Vec<Value>, Vec<&'a str>);

fn glb(
    out: &Out,
    nodes: Vec<Value>,
    (attributes, indices, targets, names): Primitive,
    skins: Option<Value>,
) -> Vec<u8> {
    let held: Vec<u64> = nodes
        .iter()
        .flat_map(|n| n["children"].as_array().into_iter().flatten())
        .filter_map(Value::as_u64)
        .collect();
    let roots: Vec<usize> = (0..nodes.len()).filter(|i| !held.contains(&(*i as u64))).collect();
    let mut doc = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": roots }],
        "nodes": nodes,
        "meshes": [{ "name": "body", "extras": { "targetNames": names },
            "primitives": [{ "attributes": attributes, "indices": indices, "targets": targets }] }],
        "buffers": [{ "byteLength": out.bin.len() }],
        "bufferViews": out.views,
        "accessors": out.accessors,
    });
    if let Some(skins) = skins {
        doc["skins"] = skins;
    }
    let mut text = doc.to_string().into_bytes();
    text.resize(text.len().div_ceil(4) * 4, b' ');
    let total = 12 + 8 + text.len() + 8 + out.bin.len();
    let mut bytes = Vec::with_capacity(total);
    for word in [0x4654_6C67, 2, total as u32, text.len() as u32, 0x4E4F_534A] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(text);
    bytes.extend((out.bin.len() as u32).to_le_bytes());
    bytes.extend(0x004E_4942u32.to_le_bytes());
    bytes.extend(&out.bin);
    bytes
}
