use super::{Edit, Kind, Op};
use crate::message::{Message, said};
use serde_json::Value;

fn shorthand(key: &str) -> Option<(&'static str, Kind, &'static str)> {
    Some(match key {
        "ev" => ("ev", Kind::Exposure { ev: 0.0 }, "ev"),
        "temperature" | "tint" => {
            let kind = Kind::WhiteBalance { temperature: 6504.0, tint: 0.0 };
            ("wb", kind, if key == "tint" { "tint" } else { "temperature" })
        }
        "contrast" => ("ct", Kind::Contrast { amount: 0.0 }, "amount"),
        "saturation" | "vibrance" => {
            let kind = Kind::Saturation { saturation: 0.0, vibrance: 0.0 };
            ("sa", kind, if key == "vibrance" { "vibrance" } else { "saturation" })
        }
        _ => return None,
    })
}

fn typed(current: &Value, text: &str) -> Result<Value, Message> {
    let text = text.trim();
    let number = |t: &str| match t.trim().parse::<i64>() {
        Ok(n) => Ok(Value::from(n)),
        Err(_) => t
            .trim()
            .parse::<f64>()
            .map(Value::from)
            .map_err(|_| said("edit.number", "%1 is not a number", &[&t])),
    };
    match current {
        _ if text == "null" => Ok(Value::Null),
        _ if text.len() > 1 && text.starts_with('"') && text.ends_with('"') => {
            serde_json::from_str(text).map_err(|e| said("edit.json", "%1", &[&e]))
        }
        Value::Bool(_) => text
            .parse()
            .map(Value::Bool)
            .map_err(|_| said("edit.boolean", "%1 is not true or false", &[&text])),
        Value::Number(_) => number(text),
        Value::Object(_) => Err(said("edit.object", "give one of its fields", &[])),
        Value::String(_) => Ok(Value::String(text.into())),
        _ if text.starts_with('[') => serde_json::from_str(text).map_err(|e| said("edit.json", "%1", &[&e])),
        Value::Array(_) => text.split(',').map(number).collect::<Result<_, _>>().map(Value::Array),
        Value::Null if text.contains(',') => {
            text.split(',').map(number).collect::<Result<_, _>>().map(Value::Array)
        }
        Value::Null => Ok(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into()))),
    }
}

fn slot<'a>(doc: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    let mut parts = key.split('.').peekable();
    let mut at = doc;
    while let Some(part) = parts.next() {
        let last = parts.peek().is_none();
        at = match at {
            Value::Array(ops) => ops.iter_mut().find(|op| op["id"] == part)?,
            Value::Object(map) => match (last, part) {
                (true, _) => map.entry(part).or_insert(Value::Null),
                (false, "span" | "depth" | "parallax") => {
                    map.entry(part).or_insert_with(|| serde_json::json!({}))
                }
                _ => map.get_mut(part)?,
            },
            _ => return None,
        };
    }
    Some(at)
}

impl Edit {
    fn resolve(&self, key: &str) -> Result<(Value, String), Message> {
        let mut next = self.clone();
        let path = match shorthand(key) {
            Some((id, kind, field)) => {
                if !next.ops.iter().any(|op| op.id == id) {
                    next.ops.push(Op { id: id.into(), enabled: true, mask: None, opacity: None, kind });
                }
                format!("ops.{id}.{field}")
            }
            None => key.to_owned(),
        };
        Ok((serde_json::to_value(&next).map_err(|e| said("edit.json", "%1", &[&e]))?, path))
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        let (mut doc, path) = self.resolve(key.trim()).ok()?;
        slot(&mut doc, &path).map(Value::take).filter(|v| !v.is_null())
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), Message> {
        let key = key.trim();
        let (mut doc, path) = self.resolve(key)?;
        let field = slot(&mut doc, &path)
            .ok_or_else(|| said("edit.unknown_setting", "%1: unknown setting", &[&key]))?;
        *field = match path.rsplit('.').next() {
            Some("mask") if value.trim().is_empty() => Value::Null,
            Some("mask") => serde_json::from_str(value.trim())
                .ok()
                .filter(Value::is_string)
                .unwrap_or_else(|| Value::String(value.trim().into())),
            _ => typed(field, value).map_err(|e| said("edit.setting", "%1: %2", &[&key, &e]).with(2, e))?,
        };
        *self = Edit::from_value(doc)?;
        Ok(())
    }
}
