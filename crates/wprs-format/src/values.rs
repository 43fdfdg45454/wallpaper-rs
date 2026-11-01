use crate::Property;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use wprs_scene::Values;

pub const VALUES_FORMAT: &str = "wpak-values/1";
const MAX_VALUES: usize = 256 << 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exported {
    pub format: String,
    pub package: String,
    #[serde(default)]
    pub version: String,
    pub values: Values,
}

pub fn portable(props: &BTreeMap<String, Property>, values: &Values) -> Values {
    let kept = |id: &String| props.get(id).is_some_and(|p| p.holds_value() && !p.personal());
    values.iter().filter(|(id, _)| kept(id)).map(|(k, v)| (k.clone(), v.clone())).collect()
}

pub fn export(package: &str, version: &str, values: Values) -> String {
    let exported =
        Exported { format: VALUES_FORMAT.into(), package: package.into(), version: version.into(), values };
    serde_json::to_string_pretty(&exported).unwrap_or_default()
}

pub fn import(
    text: &str,
    package: &str,
    props: &BTreeMap<String, Property>,
) -> Result<(Values, Vec<String>), String> {
    if text.len() > MAX_VALUES {
        return Err("the settings are larger than 256 KiB".into());
    }
    let given: Exported = serde_json::from_str(text).map_err(|e| format!("these are not settings: {e}"))?;
    if given.format != VALUES_FORMAT {
        return Err(format!("expected settings in the {VALUES_FORMAT} format"));
    }
    if given.package != package {
        return Err(format!("these settings are for {}, not {package}", given.package));
    }
    let (mut kept, mut warnings) = (Values::new(), Vec::new());
    for (id, value) in given.values {
        match props.get(&id) {
            None => warnings.push(format!("{id} is not a setting of {package}")),
            Some(p) if !p.holds_value() || p.personal() => warnings.push(format!("{id} is not imported")),
            Some(p) => match p.value(&value) {
                Ok(value) => drop(kept.insert(id, value)),
                Err(e) => warnings.push(format!("{id} {e}")),
            },
        }
    }
    Ok((kept, warnings))
}

pub fn saved(preset: &Json) -> Values {
    serde_json::from_value(preset.get("values").cloned().unwrap_or_default()).unwrap_or_default()
}

pub struct Chosen {
    pub values: Values,
    pub files: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

pub fn chosen(preset: &crate::PresetFile, props: &BTreeMap<String, Property>) -> Chosen {
    let (mut values, mut files, mut warnings) = (Values::new(), Vec::new(), Vec::new());
    let base = &preset.base.id;
    for (id, value) in &preset.values {
        match props.get(id) {
            None => warnings.push(format!("{id} is not a setting of {base}")),
            Some(p) if !p.holds_value() || p.personal() => {
                warnings.push(format!("{id} is not set by presets"))
            }
            Some(p) => match p.value(value) {
                Ok(fit) if fit == *value => drop(values.insert(id.clone(), fit)),
                Ok(_) => warnings.push(format!("{id} is out of range")),
                Err(e) => warnings.push(format!("{id} {e}")),
            },
        }
    }
    for (id, path) in &preset.files {
        let what = if crate::media::is_video_path(path) { "video" } else { "image" };
        match props.get(id).filter(|p| matches!(p.control, wprs_scene::Control::File { .. })) {
            Some(p) if p.accepts(what) => files.push((id.clone(), path.clone())),
            Some(_) => warnings.push(format!("{id} takes no {what}")),
            None => warnings.push(format!("{id} is not a file setting of {base}")),
        }
    }
    Chosen { values, files, warnings }
}
