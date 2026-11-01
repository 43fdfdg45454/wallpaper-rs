use super::Skin;
use anyhow::Result;
use serde_json::{Value, json};
use wprs_scene::mesh_math::{Mat, apply, invert};

fn floats(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

fn vec3(n: usize) -> Value {
    json!({ "componentType": 5126, "count": n, "type": "VEC3" })
}

fn columns(m: &Mat) -> [f32; 16] {
    [m[0], m[4], m[8], 0.0, m[1], m[5], m[9], 0.0, m[2], m[6], m[10], 0.0, m[3], m[7], m[11], 1.0]
}

fn linear(m: &Mat) -> Mat {
    [m[0], m[1], m[2], 0.0, m[4], m[5], m[6], 0.0, m[8], m[9], m[10], 0.0]
}

fn keep(old: &Value, wanted: &[usize]) -> Vec<usize> {
    let listed = old.as_array().into_iter().flatten().filter_map(|v| v.as_u64()).map(|v| v as usize);
    let mut out: Vec<usize> = listed.filter(|c| wanted.contains(c)).collect();
    out.extend(wanted.iter().filter(|c| !out.contains(c)).collect::<Vec<_>>());
    out
}

impl Skin {
    pub(super) fn into_glb(mut self) -> Result<Vec<u8>> {
        self.settle();
        let worlds = self.worlds();
        let joints = self.bones.clone();
        let skinned = !joints.is_empty();
        let count = self.names.len();
        let doc = &mut self.glb.json;
        for n in 0..count {
            let node = &mut doc["nodes"][n];
            let Some(map) = node.as_object_mut() else { continue };
            map.remove("matrix");
            let l = &self.locals[n];
            map.insert("name".into(), json!(self.names[n]));
            map.insert("translation".into(), json!(l.t));
            map.insert("rotation".into(), json!(l.r));
            map.insert("scale".into(), json!(l.s));
            let below: Vec<usize> = (0..count).filter(|c| self.parents[*c] == Some(n)).collect();
            match keep(&node["children"], &below) {
                c if c.is_empty() => drop(node.as_object_mut().map(|m| m.remove("children"))),
                c => node["children"] = json!(c),
            }
            if node.get("mesh").is_some() {
                match skinned {
                    true => node["skin"] = json!(0),
                    false => drop(node.as_object_mut().map(|m| m.remove("skin"))),
                }
            }
        }
        let roots: Vec<usize> = (0..count).filter(|n| self.parents[*n].is_none()).collect();
        let scene = doc["scene"].as_u64().unwrap_or(0) as usize;
        if !doc["scenes"].is_array() || doc["scenes"].as_array().is_some_and(Vec::is_empty) {
            (doc["scenes"], doc["scene"]) = (json!([{}]), json!(0));
        }
        let last = doc["scenes"].as_array().map_or(1, Vec::len) - 1;
        let slot = &mut doc["scenes"][scene.min(last)];
        slot["nodes"] = json!(keep(&slot["nodes"], &roots));
        let pieces = std::mem::take(&mut self.pieces);
        for p in &pieces {
            let back = invert(&worlds[p.node]);
            let at = |q: [f32; 3]| if skinned { q } else { apply(&back, q) };
            let points: Vec<[f32; 3]> =
                self.points[p.first..p.first + p.count].iter().map(|q| at(*q)).collect();
            let (lo, hi) = points.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), q| {
                (std::array::from_fn(|i| lo[i].min(q[i])), std::array::from_fn(|i| hi[i].max(q[i])))
            });
            let mut bounds = vec3(p.count);
            if p.count > 0 {
                (bounds["min"], bounds["max"]) = (json!(lo), json!(hi));
            }
            let position = self.glb.add(&floats(points.iter().flatten().copied()), bounds);
            let mut set = json!({ "POSITION": position });
            if skinned {
                let mut ids = Vec::with_capacity(p.count * 8);
                let mut ws = Vec::with_capacity(p.count * 4);
                for w in &self.weights[p.first..p.first + p.count] {
                    let sum: f32 = w.iter().map(|x| x.1).sum();
                    for k in 0..4 {
                        let (b, x) = w.get(k).copied().unwrap_or((joints[0], 0.0));
                        let j = joints.iter().position(|j| *j == b).unwrap_or(0) as u16;
                        ids.extend(j.to_le_bytes());
                        ws.push(if sum > 0.0 { x / sum } else { f32::from(k == 0) });
                    }
                }
                let vec4 = |kind: u32| json!({ "componentType": kind, "count": p.count, "type": "VEC4" });
                set["JOINTS_0"] = json!(self.glb.add(&ids, vec4(5123)));
                set["WEIGHTS_0"] = json!(self.glb.add(&floats(ws), vec4(5126)));
            }
            let turn = linear(&back);
            for (t, deltas) in &p.targets {
                let moved = deltas.iter().map(|d| if skinned { *d } else { apply(&turn, *d) });
                let a = self.glb.add(&floats(moved.flatten()), vec3(p.count));
                self.glb.json["meshes"][p.mesh]["primitives"][p.primitive]["targets"][*t]["POSITION"] =
                    json!(a);
            }
            let primitive = &mut self.glb.json["meshes"][p.mesh]["primitives"][p.primitive]["attributes"];
            if let Some(map) = primitive.as_object_mut() {
                ["JOINTS_0", "WEIGHTS_0", "JOINTS_1", "WEIGHTS_1"].iter().for_each(|k| drop(map.remove(*k)));
                map.extend(set.as_object().cloned().unwrap_or_default());
            }
        }
        match skinned {
            true => {
                let inverse = joints.iter().flat_map(|j| columns(&invert(&worlds[*j])));
                let mat4 = json!({ "componentType": 5126, "count": joints.len(), "type": "MAT4" });
                let ibm = self.glb.add(&floats(inverse), mat4);
                let weights = if self.auto { "auto" } else { "painted" };
                self.glb.json["skins"] = json!([{ "joints": joints, "inverseBindMatrices": ibm, "extras": { "weights": weights } }]);
            }
            false => drop(self.glb.json.as_object_mut().map(|m| m.remove("skins"))),
        }
        Ok(self.glb.bytes())
    }
}
