use crate::apply::parse_fit;
use crate::daemon::Daemon;
use crate::error::{Error, Result, invalid};
use crate::items::{preset, reply};
use crate::profiles::{sections, stored};
use std::collections::HashMap;
use wprs_core::config::parse_hex_color;
use wprs_core::model::{MODEL, PresetValue, PresetsReply};

fn placing(preset: &wprs_library::Preset) -> bool {
    preset.package().is_none()
}

fn scope(item: &str) -> Option<&str> {
    Some(item.trim()).filter(|item| !item.is_empty())
}

pub fn preset_value(values: &HashMap<String, String>) -> Result<PresetValue> {
    if values.is_empty() {
        return Err(invalid("give fit, background or both"));
    }
    let mut value = PresetValue { fit: None, background: None };
    for (key, text) in values {
        match key.as_str() {
            "fit" => value.fit = Some(parse_fit(text)?.ok_or_else(|| invalid("fit is empty"))?),
            "background" => {
                let color = parse_hex_color(text).filter(|_| text.trim().starts_with('#'));
                let [r, g, b] =
                    color.ok_or_else(|| invalid(format!("invalid color {text:?}, expected #rrggbb")))?;
                value.background = Some(format!("#{r:02x}{g:02x}{b:02x}"));
            }
            _ => return Err(invalid(format!("unknown preset key {key:?}; use fit or background"))),
        }
    }
    Ok(value)
}

impl Daemon {
    fn labelled(&self, uid: &str) -> bool {
        let mut config = self.state().config.clone();
        sections(&mut config).any(|s| s.preset.as_deref() == Some(uid))
    }

    pub fn list_presets(&self, item: &str) -> Result<String> {
        let (serial, item) = (self.changes.serial(), scope(item));
        let found = self.store.read(|lib| lib.presets(item))?.into_iter();
        let presets = found.filter(placing).map(preset).collect();
        reply(PresetsReply { model: MODEL, serial, item: item.map(String::from), presets })
    }

    pub fn save_preset(&self, item: &str, name: &str, values: &HashMap<String, String>) -> Result<String> {
        let (value, item) = (preset_value(values)?, scope(item));
        let json = stored(&value);
        let (uid, replaced) = self.store.write(|lib| {
            Ok::<_, Error>(
                match lib
                    .presets(item)?
                    .into_iter()
                    .find(|p| placing(p) && p.name.eq_ignore_ascii_case(name.trim()))
                {
                    Some(found) => {
                        lib.update_preset(&found.uid, &json)?;
                        (found.uid, true)
                    }
                    None => (lib.create_preset(item, name, &json)?, false),
                },
            )
        })?;
        if replaced && self.labelled(&uid) {
            self.update_config(|c| {
                for section in sections(c).filter(|s| s.preset.as_deref() == Some(uid.as_str())) {
                    section.fit = value.fit.or(section.fit);
                    section.background = value.background.clone().or(section.background.take());
                }
                Ok(())
            })?;
            self.configure_all();
        }
        self.changes.changed("profiles", vec![uid.clone()]);
        Ok(uid)
    }

    pub fn delete_preset(&self, uid: &str) -> Result<()> {
        let uid = uid.trim();
        self.store.write(|lib| lib.remove_preset(uid))?;
        if self.labelled(uid) {
            self.update_config(|c| {
                sections(c).filter(|s| s.preset.as_deref() == Some(uid)).for_each(|s| s.preset = None);
                Ok(())
            })?;
        }
        self.changes.changed("profiles", vec![uid.to_owned()]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wprs_core::Fit;

    #[test]
    fn preset_values_take_fit_and_background_only() {
        let values =
            |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let value = preset_value(&values(&[("fit", "center"), ("background", "#A0B0C0")])).unwrap();
        assert_eq!((value.fit, value.background.as_deref()), (Some(Fit::Center), Some("#a0b0c0")));
        assert_eq!(stored(&value), json!({"fit": "center", "background": "#a0b0c0"}));
        assert_eq!(
            stored(PresetValue { fit: None, background: value.background }),
            json!({"background": "#a0b0c0"})
        );
        for bad in [&[][..], &[("fit", "zoom")], &[("background", "a0b0c0")], &[("speed", "2")]] {
            assert!(preset_value(&values(bad)).is_err(), "{bad:?}");
        }
    }
}
