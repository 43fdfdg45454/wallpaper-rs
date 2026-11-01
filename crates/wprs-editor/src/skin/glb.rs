use anyhow::{Result, anyhow};
use serde_json::{Value, json};

pub(crate) struct Glb {
    pub json: Value,
    pub bin: Vec<u8>,
}

fn word(bytes: &[u8], at: usize) -> Option<usize> {
    bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

fn list<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v[key].as_array().into_iter().flatten()
}

impl Glb {
    pub fn read(bytes: &[u8]) -> Result<Glb> {
        let cut = || anyhow!("the mesh file is cut short");
        let len = word(bytes, 12).ok_or_else(cut)?;
        let json = serde_json::from_slice(bytes.get(20..20 + len).ok_or_else(cut)?)?;
        let at = 20 + len;
        let bin = match (word(bytes, at), word(bytes, at + 4)) {
            (Some(n), Some(0x004E_4942)) => bytes.get(at + 8..at + 8 + n).ok_or_else(cut)?.to_vec(),
            _ => Vec::new(),
        };
        Ok(Glb { json, bin })
    }

    pub fn add(&mut self, bytes: &[u8], mut accessor: Value) -> usize {
        self.bin.resize(self.bin.len().div_ceil(4) * 4, 0);
        let view = json!({ "buffer": 0, "byteOffset": self.bin.len(), "byteLength": bytes.len() });
        self.bin.extend_from_slice(bytes);
        let views = self.json["bufferViews"].as_array().map_or(0, Vec::len);
        push(&mut self.json, "bufferViews", view);
        accessor["bufferView"] = json!(views);
        push(&mut self.json, "accessors", accessor)
    }

    fn compact(&mut self) {
        let doc = &self.json;
        let mut used: Vec<u64> = Vec::new();
        for m in list(doc, "meshes") {
            for p in list(m, "primitives") {
                let targets = list(p, "targets").flat_map(|t| t.as_object().into_iter().flatten());
                let attrs = p["attributes"].as_object().into_iter().flatten().chain(targets);
                used.extend(attrs.filter_map(|(_, a)| a.as_u64()).chain(p["indices"].as_u64()));
            }
        }
        used.extend(list(doc, "skins").filter_map(|s| s["inverseBindMatrices"].as_u64()));
        let samplers = list(doc, "animations").flat_map(|a| list(a, "samplers"));
        used.extend(samplers.flat_map(|s| [s["input"].as_u64(), s["output"].as_u64()]).flatten());
        let accessors: Vec<Value> = list(doc, "accessors").cloned().collect();
        let kept: Vec<usize> = (0..accessors.len()).filter(|i| used.contains(&(*i as u64))).collect();
        let index = |i: u64| kept.iter().position(|k| *k as u64 == i).map_or(json!(0), |k| json!(k));
        let mut views: Vec<u64> = kept.iter().filter_map(|k| accessors[*k]["bufferView"].as_u64()).collect();
        views.extend(list(doc, "images").filter_map(|i| i["bufferView"].as_u64()));
        views.sort_unstable();
        views.dedup();
        let old: Vec<Value> = list(doc, "bufferViews").cloned().collect();
        let mut bin = Vec::new();
        let mut fresh = Vec::new();
        for v in &views {
            let mut view = old.get(*v as usize).cloned().unwrap_or(json!({}));
            let (at, n) =
                (view["byteOffset"].as_u64().unwrap_or(0), view["byteLength"].as_u64().unwrap_or(0));
            bin.resize(bin.len().div_ceil(4) * 4, 0);
            view["byteOffset"] = json!(bin.len());
            view["buffer"] = json!(0);
            bin.extend_from_slice(self.bin.get(at as usize..(at + n) as usize).unwrap_or_default());
            fresh.push(view);
        }
        let place = |v: &mut Value| {
            if let Some(i) = v["bufferView"].as_u64() {
                v["bufferView"] = json!(views.iter().position(|x| *x == i).unwrap_or(0));
            }
        };
        let mut moved: Vec<Value> = kept.iter().map(|k| accessors[*k].clone()).collect();
        moved.iter_mut().for_each(place);
        let doc = &mut self.json;
        for m in items(doc, "meshes") {
            for p in items(m, "primitives") {
                let targets = items(p, "targets");
                for t in targets.flat_map(|t| t.as_object_mut().into_iter().flatten()) {
                    *t.1 = index(t.1.as_u64().unwrap_or(0));
                }
                for (_, a) in p["attributes"].as_object_mut().into_iter().flatten() {
                    *a = index(a.as_u64().unwrap_or(0));
                }
                if let Some(i) = p["indices"].as_u64() {
                    p["indices"] = index(i);
                }
            }
        }
        for s in items(doc, "skins") {
            if let Some(i) = s["inverseBindMatrices"].as_u64() {
                s["inverseBindMatrices"] = index(i);
            }
        }
        for a in items(doc, "animations") {
            for s in items(a, "samplers") {
                for key in ["input", "output"] {
                    s[key] = index(s[key].as_u64().unwrap_or(0));
                }
            }
        }
        items(doc, "images").for_each(place);
        (doc["accessors"], doc["bufferViews"]) = (json!(moved), json!(fresh));
        doc["buffers"] = json!([{ "byteLength": bin.len().max(4) }]);
        bin.resize(bin.len().max(4).div_ceil(4) * 4, 0);
        self.bin = bin;
    }

    pub fn bytes(mut self) -> Vec<u8> {
        self.compact();
        let mut text = self.json.to_string().into_bytes();
        text.resize(text.len().div_ceil(4) * 4, b' ');
        let total = 12 + 8 + text.len() + 8 + self.bin.len();
        let mut out = Vec::with_capacity(total);
        for w in [0x4654_6C67, 2, total as u32, text.len() as u32, 0x4E4F_534A] {
            out.extend(w.to_le_bytes());
        }
        out.extend(text);
        out.extend((self.bin.len() as u32).to_le_bytes());
        out.extend(0x004E_4942u32.to_le_bytes());
        out.extend(&self.bin);
        out
    }
}

pub(crate) fn items<'a>(v: &'a mut Value, key: &str) -> impl Iterator<Item = &'a mut Value> {
    v.get_mut(key).and_then(Value::as_array_mut).into_iter().flatten()
}

fn push(doc: &mut Value, key: &str, value: Value) -> usize {
    if !doc[key].is_array() {
        doc[key] = json!([]);
    }
    let items = doc[key].as_array_mut().expect("a list");
    items.push(value);
    items.len() - 1
}
