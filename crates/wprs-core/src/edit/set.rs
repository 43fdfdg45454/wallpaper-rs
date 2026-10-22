use super::{Edit, Kind, Op};
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

fn typed(current: &Value, text: &str) -> Result<Value, String> {
    let text = text.trim();
    let number = |t: &str| match t.trim().parse::<i64>() {
        Ok(n) => Ok(Value::from(n)),
        Err(_) => t.trim().parse::<f64>().map(Value::from).map_err(|_| format!("{t} is not a number")),
    };
    match current {
        _ if text == "null" => Ok(Value::Null),
        Value::Bool(_) => text.parse().map(Value::Bool).map_err(|_| format!("{text} is not true or false")),
        Value::Number(_) => number(text),
        Value::Object(_) => Err("give one of its fields".into()),
        Value::String(_) => Ok(Value::String(text.into())),
        _ if text.starts_with('[') => serde_json::from_str(text).map_err(|e| e.to_string()),
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
                (false, "span") => map.entry(part).or_insert_with(|| serde_json::json!({})),
                _ => map.get_mut(part)?,
            },
            _ => return None,
        };
    }
    Some(at)
}

impl Edit {
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let key = key.trim();
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
        let mut doc = serde_json::to_value(&next).map_err(|e| e.to_string())?;
        let field = slot(&mut doc, &path).ok_or_else(|| format!("{key}: unknown setting"))?;
        *field = typed(field, value).map_err(|e| format!("{key}: {e}"))?;
        *self = Edit::from_value(doc)?;
        Ok(())
    }
}
