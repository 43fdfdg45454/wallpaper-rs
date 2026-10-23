use crate::{Library, Preset, Result};
use serde_json::{Value, json};

impl Preset {
    pub fn package(&self) -> Option<&str> {
        self.value.get("package").and_then(Value::as_str)
    }

    pub fn values(&self) -> Value {
        self.value.get("values").cloned().unwrap_or_else(|| json!({}))
    }
}

impl Library {
    pub fn settings(&self, package: &str) -> Result<Vec<Preset>> {
        Ok(self.presets(None)?.into_iter().filter(|p| p.package() == Some(package)).collect())
    }

    pub fn keep_settings(&mut self, package: &str, name: &str, values: &Value) -> Result<String> {
        let value = json!({ "package": package, "values": values });
        match self.settings(package)?.into_iter().find(|p| p.name.eq_ignore_ascii_case(name.trim())) {
            Some(found) => self.update_preset(&found.uid, &value).map(|()| found.uid),
            None => self.create_preset(None, name, &value),
        }
    }
}
