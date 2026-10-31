use crate::message::{Message, said};
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_path_to_error::Segment;

fn found(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("invalid type: ").or_else(|| message.strip_prefix("invalid value: "))?;
    rest.split_once(", expected").map(|(found, _)| found)
}

fn shows(value: &Value, found: &str) -> bool {
    match value {
        Value::String(s) => found == format!("string {s:?}"),
        Value::Bool(b) => found == format!("boolean `{b}`"),
        Value::Number(n) => found.ends_with(&format!("`{n}`")),
        Value::Array(_) => found == "sequence",
        Value::Object(_) => found == "map",
        Value::Null => found == "null" || found == "unit value",
    }
}

pub(super) fn typed<T: DeserializeOwned>(value: &Value) -> Result<T, Message> {
    serde_path_to_error::deserialize(value).map_err(|e| {
        let path = e.path().clone();
        let inner = e.into_inner().to_string();
        let at = path.iter().try_fold(value, |v, segment| match segment {
            Segment::Seq { index } => v.get(index),
            Segment::Map { key } => v.get(key),
            _ => Some(v),
        });
        let field = at.and_then(Value::as_object).zip(found(&inner)).and_then(|(map, found)| {
            map.iter().find(|(key, v)| *key != "type" && shows(v, found)).map(|(key, _)| key.clone())
        });
        let path = Some(path.to_string()).filter(|p| p != ".").into_iter().chain(field);
        match path.collect::<Vec<_>>().join(".") {
            path if path.is_empty() => said("edit.json", "%1", &[&inner]),
            path => said("edit.field", "%1: %2", &[&path, &inner]),
        }
    })
}
