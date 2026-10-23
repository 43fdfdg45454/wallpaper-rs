use crate::daemon::Daemon;
use crate::error::{Result, invalid, not_found};
use crate::sceneprops::Props;
use wprs_format::values;
use wprs_library::Preset;
use wprs_scene::Values;

impl Daemon {
    pub fn user_presets(&self, package: &str) -> Result<Vec<Preset>> {
        self.store.read(|lib| lib.settings(package))
    }

    fn preset_values(&self, props: &Props, key: &str) -> Result<Values> {
        let key = key.trim();
        let label = |p: &wprs_scene::Preset| {
            p.label.as_ref().is_some_and(|l| l.pick(&props.languages).eq_ignore_ascii_case(key))
        };
        if let Some(preset) =
            props.presets.iter().find(|p| p.id == key).or_else(|| props.presets.iter().find(|p| label(p)))
        {
            return Ok(preset.values.clone());
        }
        let mine = self.user_presets(&props.id)?.into_iter();
        let found = mine.into_iter().find(|p| p.uid == key || p.name.eq_ignore_ascii_case(key));
        let found = found.ok_or_else(|| not_found(format!("{} has no preset {key:?}", props.id)))?;
        Ok(values::saved(&found.value))
    }

    pub fn apply_scene_preset(&self, output: &str, preset: &str) -> Result<()> {
        let (output, props) = self.props_on(output)?;
        let values = self.preset_values(&props, preset)?;
        self.keep_values(&output, &props, &values)
    }

    fn current(&self, output: &str, props: &Props) -> Values {
        let st = self.state();
        values::portable(&props.properties, &props.values(&st.config.scene_values(output, &props.id)))
    }

    pub fn save_scene_preset(&self, output: &str, name: &str) -> Result<String> {
        let (output, props) = self.props_on(output)?;
        let values = self.current(&output, &props);
        self.keep_preset(&props, name, &values)
    }

    fn keep_preset(&self, props: &Props, name: &str, values: &Values) -> Result<String> {
        let values = serde_json::to_value(values).unwrap_or_default();
        let uid = self.store.write(|lib| lib.keep_settings(&props.id, name, &values))?;
        self.changes.changed("profiles", vec![uid.clone()]);
        Ok(uid)
    }

    fn scene_preset(&self, uid: &str) -> Result<Preset> {
        let preset = self.store.read(|lib| lib.preset(uid.trim()))?;
        let settings = preset.package().is_some();
        settings.then_some(preset).ok_or_else(|| invalid(format!("{uid} is not a preset of settings")))
    }

    pub fn rename_scene_preset(&self, uid: &str, name: &str) -> Result<()> {
        let preset = self.scene_preset(uid)?;
        self.store.write(|lib| lib.rename_preset(&preset.uid, name))?;
        self.changes.changed("profiles", vec![preset.uid]);
        Ok(())
    }

    pub fn delete_scene_preset(&self, uid: &str) -> Result<()> {
        let preset = self.scene_preset(uid)?;
        self.store.write(|lib| lib.remove_preset(&preset.uid))?;
        self.changes.changed("profiles", vec![preset.uid]);
        Ok(())
    }

    pub fn export_scene_values(&self, output: &str, preset: &str) -> Result<String> {
        let (output, props) = self.props_on(output)?;
        let chosen = match preset.trim() {
            "" => self.current(&output, &props),
            key => values::portable(&props.properties, &self.preset_values(&props, key)?),
        };
        Ok(values::export(&props.id, &props.version, chosen))
    }

    pub fn import_scene_values(&self, output: &str, text: &str, name: &str) -> Result<Vec<String>> {
        let (output, props) = self.props_on(output)?;
        let (kept, warnings) = values::import(text, &props.id, &props.properties).map_err(invalid)?;
        match name.trim() {
            "" => self.keep_values(&output, &props, &kept)?,
            name => drop(self.keep_preset(&props, name, &kept)?),
        }
        Ok(warnings)
    }
}
