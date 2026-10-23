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
