use crate::ops::{Op, escape, set};
use serde_json::{Value, json};

const DATA: [&str; 3] = ["normal", "roughness_map", "emissive_map"];

pub(crate) fn uses(doc: &Value, id: &str) -> Vec<(String, bool)> {
    fn walk(v: &Value, at: &str, key: &str, id: &str, out: &mut Vec<(String, bool)>) {
        match v {
            Value::String(s) if s == id && key != "id" => out.push((at.into(), DATA.contains(&key))),
            Value::Object(_) if at == "/scene/assets" => {}
            Value::Object(m) => {
                m.iter().for_each(|(k, v)| walk(v, &format!("{at}/{}", escape(k)), k, id, out));
            }
            Value::Array(l) => {
                l.iter().enumerate().for_each(|(i, v)| walk(v, &format!("{at}/{i}"), key, id, out));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(doc, "", "", id, &mut out);
    out
}

pub(crate) fn only_at(doc: &Value, id: &str, here: &str) -> bool {
    uses(doc, id).iter().all(|(at, _)| at == here)
}

pub(crate) fn linear(doc: &Value, id: &str, taken: &mut Vec<(String, String)>) -> (String, Vec<Op>) {
    if let Some((_, twin)) = taken.iter().find(|(from, _)| from == id) {
        return (twin.clone(), Vec::new());
    }
    let asset = doc.pointer(&format!("/scene/assets/{}", escape(id))).cloned().unwrap_or(Value::Null);
    let (twin, ops) = match () {
        _ if asset["linear"] == true => (id.to_string(), Vec::new()),
        _ if uses(doc, id).iter().all(|(_, data)| *data) => {
            (id.to_string(), set(doc, &format!("/scene/assets/{}/linear", escape(id)), json!(true)))
        }
        _ => {
            let twin = crate::layers::unique(doc, &format!("{id}_linear"));
            let mut copy = asset;
            copy["linear"] = json!(true);
            let ops = set(doc, &format!("/scene/assets/{}", escape(&twin)), copy);
            (twin, ops)
        }
    };
    taken.push((id.to_string(), twin.clone()));
    (twin, ops)
}
