use crate::account::DOWNLOADS;
use crate::apply::target_output;
use crate::daemon::Daemon;
use crate::error::{Error, Result, invalid};
use crate::items::reply;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use wprs_core::model::{MODEL, Profile, ProfileAuto, ProfileOutput, ProfileReply, ProfilesReply};
use wprs_core::{Config, OutputConfig};

pub fn stored(value: impl Serialize) -> Value {
    fn strip(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                map.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, strip(v))).collect()
            }
            other => other,
        }
    }
    strip(serde_json::to_value(value).unwrap_or_default())
}

pub fn sections(c: &mut Config) -> impl Iterator<Item = &mut OutputConfig> {
    std::iter::once(&mut c.default).chain(c.outputs.values_mut())
}

pub fn auto(value: &Value) -> ProfileAuto {
    serde_json::from_value(value.clone()).unwrap_or(ProfileAuto { outputs: None, on_battery: None })
}

pub(crate) fn model(p: wprs_library::Profile, active: Option<&String>) -> Profile {
    let (outputs, options) = (serde_json::from_value(p.outputs), serde_json::from_value(p.options));
    let (active, auto) = (active == Some(&p.uid), auto(&p.auto));
    let (outputs, options) = (outputs.unwrap_or_default(), options.unwrap_or_default());
    Profile { uid: p.uid, name: p.name, outputs, options, auto, priority: p.priority, active }
}

fn content(o: &OutputConfig, items: &HashMap<PathBuf, String>) -> ProfileOutput {
    let item = o.image.as_ref().and_then(|path| items.get(path).cloned());
    let (path, playlist, fit) = (o.image.clone().filter(|_| item.is_none()), o.playlist.clone(), o.fit);
    let (background, preset, filter) = (o.background.clone(), o.preset.clone(), o.filter);
    ProfileOutput { item, path, playlist, fit, background, preset, filter }
}

impl Daemon {
    pub fn list_profiles(&self) -> Result<String> {
        let (serial, active) = (self.changes.serial(), self.state().config.profile.clone());
        let profiles =
            self.store.read(|lib| lib.profiles())?.into_iter().map(|p| model(p, active.as_ref())).collect();
        reply(ProfilesReply { model: MODEL, serial, active, profiles })
    }

    pub fn get_profile(&self, key: &str) -> Result<String> {
        let (serial, active) = (self.changes.serial(), self.state().config.profile.clone());
        let profile = model(self.store.read(|lib| lib.profile(key))?, active.as_ref());
        reply(ProfileReply { model: MODEL, serial, profile })
    }

    pub fn save_profile(&self, name: &str, with_options: bool) -> Result<String> {
        let (config, items) = {
            let st = self.state();
            (st.config.clone(), st.items.clone())
        };
        let sections = std::iter::once((String::new(), &config.default))
            .chain(config.outputs.iter().map(|(n, o)| (n.clone(), o)));
        let outputs: BTreeMap<String, ProfileOutput> =
            sections.map(|(name, o)| (name, content(o, &items))).collect();
        let text = |value: Value| value.as_str().map_or_else(|| value.to_string(), String::from);
        let options = serde_json::to_value(config.options()).ok().and_then(|v| v.as_object().cloned());
        let options = options
            .into_iter()
            .flatten()
            .filter(|(k, _)| with_options && k != DOWNLOADS && k != wprs_sync::GPU);
        let options: BTreeMap<String, String> = options.map(|(k, v)| (k, text(v))).collect();
        let (outputs, options) = (stored(&outputs), stored(&options));
        let uid = self.store.write(|lib| {
            match lib.profiles()?.into_iter().find(|p| p.name.eq_ignore_ascii_case(name.trim())) {
                Some(p) => {
                    let uid = p.uid.clone();
                    lib.update_profile(&wprs_library::Profile { outputs, options, ..p })?;
                    Ok::<_, Error>(uid)
                }
                None => {
                    Ok(lib.create_profile(name, &outputs, &options, &Value::Object(Default::default()), 0)?)
                }
            }
        })?;
        self.changes.changed("profiles", vec![uid.clone()]);
        Ok(uid)
    }

    pub fn edit_profile(
        &self,
        key: &str,
        change: impl FnOnce(&mut wprs_library::Profile) -> Result<()>,
    ) -> Result<()> {
        let uid = self.store.write(|lib| {
            let mut profile = lib.profile(key)?;
            change(&mut profile)?;
            lib.update_profile(&profile)?;
            Ok::<_, Error>(profile.uid)
        })?;
        self.changes.changed("profiles", vec![uid]);
        Ok(())
    }

    pub fn set_auto(&self, key: &str, rule: &str, value: &str) -> Result<()> {
        let value = value.trim();
        let bad = || invalid(format!("invalid value {value:?} for {rule}"));
        self.edit_profile(key, |p| {
            let mut conditions = auto(&p.auto);
            match rule {
                "outputs" if value.is_empty() => conditions.outputs = None,
                "outputs" => {
                    let names = value.split(',').map(str::trim).filter(|n| !n.is_empty());
                    let names = names.map(target_output).collect::<Result<BTreeSet<_>>>()?;
                    conditions.outputs = Some(names.into_iter().collect()).filter(|n: &Vec<_>| !n.is_empty());
                    conditions.outputs.as_ref().ok_or_else(bad)?;
                }
                "on_battery" if value.is_empty() => conditions.on_battery = None,
                "on_battery" => conditions.on_battery = Some(value.parse().map_err(|_| bad())?),
                "priority" => {
                    p.priority = value.parse().ok().filter(|n| (-1000..=1000).contains(n)).ok_or_else(bad)?
                }
                _ => {
                    return Err(invalid(format!(
                        "unknown rule {rule:?}; the rules are listed in GetSchema().auto"
                    )));
                }
            }
            p.auto = stored(&conditions);
            Ok(())
        })
    }

    pub fn delete_profile(&self, key: &str) -> Result<()> {
        let uid = self.store.write(|lib| {
            let uid = lib.profile(key)?.uid;
            lib.remove_profile(&uid)?;
            Ok::<_, Error>(uid)
        })?;
        if self.state().config.profile.as_ref() == Some(&uid) {
            self.update_config(|c| {
                c.profile = None;
                Ok(())
            })?;
        }
        self.changes.changed("profiles", vec![uid]);
        Ok(())
    }
}
