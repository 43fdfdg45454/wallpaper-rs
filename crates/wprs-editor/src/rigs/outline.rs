use super::round;
use crate::skin::Skin;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub fn outline(dir: &Path, doc: &Value, id: &str) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Ok((_, skin)) = Skin::load(dir, doc, layer) else { return Value::Null };
    let mut out = Map::new();
    for p in &skin.pieces {
        let name = skin.glb.json["meshes"][p.mesh]["name"].as_str().filter(|n| !n.is_empty());
        let name = name.map_or(format!("mesh_{}", p.mesh), String::from);
        let inside = |t: &[u32; 3]| t.iter().all(|v| (p.first..p.first + p.count).contains(&(*v as usize)));
        let mut edges: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        for t in skin.triangles.iter().filter(|t| inside(t)) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let at = |v: u32| skin.picture(skin.points[v as usize]).map(round);
        let rim = edges.into_iter().filter(|(_, n)| *n == 1).map(|((a, b), _)| json!([at(a), at(b)]));
        let slot = out.entry(name).or_insert_with(|| json!([]));
        if let Some(list) = slot.as_array_mut() {
            list.extend(rim);
        }
    }
    Value::Object(out)
}
