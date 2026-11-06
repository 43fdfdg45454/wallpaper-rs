use serde_json::Value;

const TOUCHES: [&str; 3] = ["click", "enter", "leave"];

fn named(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(list) = map.get("events").and_then(Value::as_array) {
                out.extend(list.iter().filter_map(|e| e["name"].as_str()).map(|n| format!("event:{n}")));
            }
            map.values().for_each(|v| named(v, out));
        }
        Value::Array(list) => list.iter().for_each(|v| named(v, out)),
        _ => {}
    }
}

fn touchable(layer: &Value) -> bool {
    let solid = layer["solid"] == true || layer["solid"].is_object();
    solid || layer["hit"].as_array().is_some_and(|h| !h.is_empty())
}

pub fn offered(doc: &Value) -> Vec<String> {
    let mut events = Vec::new();
    named(&doc["scene"], &mut events);
    events.sort();
    events.dedup();
    let layers = crate::layers::walk(doc);
    let ids = layers.iter().filter(|(_, l)| touchable(l)).filter_map(|(_, l)| l["id"].as_str());
    let touches = ids.flat_map(|id| TOUCHES.map(|k| format!("{k}:{id}")));
    events.into_iter().chain(touches).collect()
}
