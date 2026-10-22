use crate::{Color, Scene, SceneError};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::mem::discriminant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Number(f32),
    Color(Color),
    Bool(bool),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Property {
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub kind: PropertyKind,
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PropertyKind {
    Color { value: Color },
    Slider { value: f32, min: f32, max: f32, step: f32 },
    Bool { value: bool },
    Combo { value: String, options: Vec<String> },
    Text { value: String },
}

impl Property {
    pub fn value(&self) -> Value {
        match &self.kind {
            PropertyKind::Color { value } => Value::Color(*value),
            PropertyKind::Slider { value, .. } => Value::Number(*value),
            PropertyKind::Bool { value } => Value::Bool(*value),
            PropertyKind::Combo { value, .. } | PropertyKind::Text { value } => Value::Text(value.clone()),
        }
    }
}

impl Value {
    pub fn lerp(&self, other: &Value, f: f32) -> Value {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Value::Number(a + (b - a) * f),
            (Value::Color(Color(a)), Value::Color(Color(b))) => {
                Value::Color(Color(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f)))
            }
            _ if f < 1.0 => self.clone(),
            _ => other.clone(),
        }
    }
}

impl Scene {
    pub fn get(&self, path: &str) -> Option<Value> {
        slot(&serde_json::to_value(self).ok()?, path)
    }

    pub fn with<'a>(&self, edits: impl IntoIterator<Item = (&'a str, Value)>) -> Result<Scene, SceneError> {
        let mut json = serde_json::to_value(self)?;
        for (path, value) in edits {
            let fits = fits(&json, path, &value);
            let target =
                json.pointer_mut(path).filter(|_| fits).ok_or_else(|| SceneError::Binding(path.into()))?;
            *target = match value {
                Value::Number(n) if target.is_u64() => json!(n.max(0.0).round() as u64),
                value => json!(value),
            };
        }
        Ok(serde_json::from_value(json)?)
    }

    pub fn with_properties(&self) -> Result<Scene, SceneError> {
        self.with(self.properties.iter().flat_map(|p| p.targets.iter().map(|t| (t.as_str(), p.value()))))
    }
}

pub(crate) fn fits(json: &Json, path: &str, value: &Value) -> bool {
    slot(json, path).is_some_and(|old| discriminant(&old) == discriminant(value))
}

fn slot(json: &Json, path: &str) -> Option<Value> {
    let leaf = path.rsplit('/').next()?;
    let bindable = path.starts_with("/layers/") && !matches!(leaf, "type" | "id" | "blend");
    bindable.then(|| Value::deserialize(json.pointer(path)?).ok()).flatten()
}
