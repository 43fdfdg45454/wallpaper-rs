use crate::apply::target_output;
use crate::daemon::Daemon;
use crate::error::{Result, invalid, not_found};
use crate::items::reply;
use crate::scenes::ScenePack;
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::model::{MODEL, SceneProperties};
use wprs_scene::Control;

type Stored = BTreeMap<String, toml::Value>;

fn json(value: impl serde::Serialize) -> Json {
    serde_json::to_value(value).unwrap_or_default()
}

fn stored(value: &Json) -> Option<toml::Value> {
    (!value.is_null()).then(|| toml::Value::try_from(value).ok()).flatten()
}

impl Daemon {
    fn scene_on(&self, output: &str) -> Result<(String, Arc<ScenePack>)> {
        let output = target_output(output)?;
        let shown = {
            let st = self.state();
            let info = st.outputs.get(&output).filter(|_| !output.is_empty());
            let path = if output.is_empty() {
                st.config.default.image.clone()
            } else {
                st.config.resolve(&output).image
            };
            info.and_then(|i| i.scene.clone()).map(Ok).or(path.map(Err))
        };
        let missing = || {
            not_found(format!(
                "{} shows no scene",
                if output.is_empty() { "the default section" } else { &output }
            ))
        };
        let pack = match shown {
            Some(Ok(pack)) => pack,
            Some(Err(path)) if wprs_library::Kind::detect(&path) == Some(wprs_library::Kind::Package) => {
                self.scene_pack(&path).map_err(|_| missing())?
            }
            _ => return Err(missing()),
        };
        Ok((output, pack))
    }

    pub fn scene_properties(&self, output: &str) -> Result<String> {
        let (output, pack) = self.scene_on(output)?;
        let values = pack.values(&self.state().config.scene_values(&output, &pack.id));
        let languages = wprs_core::locale::languages();
        let (fields, preset_labels) = wprs_scene::form(&pack.properties, &pack.presets, &values, &languages);
        let properties = pack.properties.iter().map(|(k, p)| (k.clone(), json(p))).collect();
        let presets = pack.presets.iter().map(json).collect();
        let (package, title, serial) = (pack.id.clone(), pack.title.clone(), self.changes.serial());
        reply(SceneProperties {
            model: MODEL,
            serial,
            output,
            package,
            title,
            properties,
            values,
            presets,
            fields,
            preset_labels,
        })
    }

    pub fn set_scene_property(&self, output: &str, id: &str, value: &str) -> Result<()> {
        let (output, pack) = self.scene_on(output)?;
        let prop =
            pack.properties.get(id).ok_or_else(|| invalid(format!("{} has no property {id:?}", pack.id)))?;
        let given: Json =
            serde_json::from_str(value).map_err(|e| invalid(format!("the value is not JSON: {e}")))?;
        let checked = prop.value(&given).map_err(|e| invalid(format!("{id} {e}")))?;
        if checked != given {
            tracing::warn!(property = id, "a scene property value was adjusted to its range");
        }
        if let (Control::File { .. }, Some(file)) = (&prop.control, checked.as_str()) {
            self.scene_file(file).map_err(|e| invalid(format!("{id}: cannot use {file}: {e:#}")))?;
        }
        self.store_scene(&output, &pack, |values| match stored(&checked) {
            Some(value) => drop(values.insert(id.to_owned(), value)),
            None => drop(values.remove(id)),
        })
    }

    pub fn apply_scene_preset(&self, output: &str, preset: &str) -> Result<()> {
        let (output, pack) = self.scene_on(output)?;
        let preset = pack.presets.iter().find(|p| p.id == preset.trim());
        let preset =
            preset.ok_or_else(|| not_found(format!("{} has no preset {preset:?}", pack.id)))?.clone();
        self.store_scene(&output, &pack, |values| {
            preset
                .values
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), stored(v)?)))
                .for_each(|(k, v)| drop(values.insert(k, v)))
        })
    }

    pub fn reset_scene_properties(&self, output: &str) -> Result<()> {
        let (output, pack) = self.scene_on(output)?;
        self.store_scene(&output, &pack, Stored::clear)
    }

    fn store_scene(&self, output: &str, pack: &ScenePack, change: impl FnOnce(&mut Stored)) -> Result<()> {
        self.update_config(|c| {
            let section = c.scene_section(output);
            let values = section.entry(pack.id.clone()).or_default();
            change(values);
            if values.is_empty() {
                section.remove(&pack.id);
            }
            Ok(())
        })?;
        let shows =
            |info: &crate::streams::OutputInfo| info.scene.as_ref().is_some_and(|s| s.path == pack.path);
        let affected: Vec<String> =
            self.state().outputs.iter().filter(|(_, i)| shows(i)).map(|(n, _)| n.clone()).collect();
        affected.iter().for_each(|o| drop(self.configure_output(o)));
        Ok(())
    }
}
