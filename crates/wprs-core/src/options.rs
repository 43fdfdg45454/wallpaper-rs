use crate::config::Config;
use crate::settings::fps;
use std::collections::BTreeMap;

const OPTION_SECTIONS: [&str; 6] = ["playback", "transition", "pause", "library", "repos", "models"];

impl Config {
    pub fn options(&self) -> BTreeMap<String, toml::Value> {
        let doc = toml::Value::try_from(self).unwrap_or(toml::Value::Table(Default::default()));
        OPTION_SECTIONS
            .iter()
            .filter_map(|section| Some((section, doc.get(section)?.as_table()?)))
            .flat_map(|(section, table)| {
                table.iter().map(move |(k, v)| (format!("{section}.{k}"), v.clone()))
            })
            .chain([("layout.bezel_mm".into(), toml::Value::Float(self.layout.bezel_mm))])
            .chain([("render.gpu".into(), toml::Value::String(self.render.gpu.as_str().into()))])
            .collect()
    }

    pub fn set_option(&mut self, key: &str, value: &str) -> Result<(), String> {
        if let Some(id) = key.strip_prefix("layout.monitors.") {
            return self.layout.place(id, value);
        }
        let unknown = || format!("unknown setting '{key}', see the list of settings");
        let current = self.options().remove(key).ok_or_else(unknown)?;
        let text = value.trim();
        let parsed = match current {
            toml::Value::Boolean(_) => text.parse().map(toml::Value::Boolean).map_err(|e| e.to_string()),
            toml::Value::Integer(_) => match text.parse() {
                Ok(number) => Ok(toml::Value::Integer(number)),
                Err(_) if text == fps::WORD => Ok(text.into()),
                Err(e) => Err(e.to_string()),
            },
            toml::Value::Float(_) => text.parse().map(toml::Value::Float).map_err(|e| e.to_string()),
            _ => Ok(toml::Value::String(text.to_owned())),
        }
        .map_err(|e| format!("invalid value for {key}: {e}"))?;
        let (section, field) = key.split_once('.').ok_or_else(unknown)?;
        let mut doc = toml::Value::try_from(&*self).map_err(|e| e.to_string())?;
        let table = doc.as_table_mut().map(|d| d.entry(section).or_insert_with(|| toml::Table::new().into()));
        table.and_then(toml::Value::as_table_mut).ok_or_else(unknown)?.insert(field.into(), parsed);
        let mut updated: Config = doc.try_into().map_err(|e| format!("invalid value for {key}: {e}"))?;
        if updated.normalize().iter().any(|fixed| fixed == key) {
            return Err(format!("{text} is out of range for {key}"));
        }
        *self = updated;
        Ok(())
    }
}
