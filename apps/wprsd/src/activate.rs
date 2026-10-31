use crate::account::DOWNLOADS;
use crate::daemon::Daemon;
use crate::error::{Error, Result};
use crate::profiles::model;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use wprs_core::OutputConfig;
use wprs_core::config::parse_hex_color;
use wprs_core::model::Profile;

const HELD: &str = "the profile left out a wallpaper that flashes or shows fast patterns that can trigger \
    seizures in people with photosensitive epilepsy; the profile shows it once you allow it";

impl Daemon {
    pub fn activate(&self, key: &str) -> Result<()> {
        if key.trim().is_empty() {
            self.update_config(|c| {
                c.profile = None;
                Ok(())
            })?;
            self.changes.changed("profiles", Vec::new());
            return Ok(());
        }
        let _packages = crate::compositor::lock(&crate::packages::PACKAGES);
        let profile = model(self.store.read(|lib| lib.profile(key.trim()))?, None);
        let (mut sections, held) = self.plan(&profile)?;
        self.update_config(|c| {
            sections.iter_mut().for_each(|(name, section)| section.scenes = c.scene_section(name).clone());
            (c.default, c.profile) = (sections.remove("").unwrap_or_default(), Some(profile.uid.clone()));
            c.outputs = sections;
            for (key, value) in profile.options.iter().filter(|(key, _)| *key != DOWNLOADS) {
                if let Err(e) = c.set_option(key, value) {
                    tracing::warn!(key, "skipped a profile setting: {e}");
                }
            }
            Ok(())
        })?;
        self.state().held = held;
        self.configure_all();
        self.watch_sources();
        self.changes.changed("profiles", vec![profile.uid]);
        Ok(())
    }

    pub(crate) fn allow_held(&self, key: &str) -> Result<()> {
        let _packages = crate::compositor::lock(&crate::packages::PACKAGES);
        let profile = model(self.store.read(|lib| lib.profile(key))?, None);
        let (mut sections, still) = self.plan(&profile)?;
        let freed: Vec<String> =
            self.state().held.keys().filter(|o| !still.contains_key(*o)).cloned().collect();
        if freed.is_empty() {
            return Ok(());
        }
        self.update_config(|c| {
            for (name, mut section) in freed.iter().filter_map(|o| Some((o, sections.remove(o)?))) {
                section.scenes = c.scene_section(name).clone();
                match name.is_empty() {
                    true => c.default = section,
                    false => drop(c.outputs.insert(name.clone(), section)),
                }
            }
            Ok(())
        })?;
        self.state().held.retain(|o, _| !freed.contains(o));
        let all = freed.iter().any(String::is_empty) || crate::edits::spans(&self.state());
        match all {
            true => self.configure_all(),
            false => freed.iter().for_each(|o| drop(self.configure_output(o))),
        }
        self.watch_sources();
        Ok(())
    }

    fn plan(&self, profile: &Profile) -> Result<(BTreeMap<String, OutputConfig>, HashMap<String, String>)> {
        let found = self.store.read(|lib| {
            let found = profile.outputs.iter().map(|(name, o)| {
                let content = match (&o.item, &o.path, &o.playlist) {
                    (Some(uid), ..) => {
                        lib.item(uid).ok().filter(|i| !i.missing).map(|i| (Some(i.path), None))
                    }
                    (_, Some(path), _) => Some((Some(path.clone()), None)),
                    (.., Some(uid)) => lib.playlist(uid).ok().map(|p| (None, Some(p.uid))),
                    _ => Some((None, None)),
                };
                let image = content.as_ref().and_then(|(image, _)| image.as_ref()?.to_str());
                let item = image.and_then(|path| lib.item(&lib.resolve_item(path).ok()?).ok());
                let flashing = item.and_then(|i| i.package).is_some_and(|p| p.asks("photosensitive"));
                let preset = o.preset.clone().filter(|u| lib.preset(u).is_ok());
                (name.clone(), content.filter(|_| !flashing), preset, flashing)
            });
            Ok::<_, Error>(found.collect::<Vec<_>>())
        })?;
        let (revoked, mut sections, mut held) = (self.revoked(), BTreeMap::new(), HashMap::new());
        for (name, content, preset, flashing) in found {
            let usable = |(image, _): &(Option<PathBuf>, Option<String>)| {
                image.as_ref().is_none_or(|p| !revoked.contains(p) && self.check_media(p).is_ok())
            };
            let content = content.filter(usable);
            if content.is_none() {
                tracing::warn!(
                    output = name,
                    "skipped a profile entry whose item, file or playlist is not usable"
                );
            }
            if flashing {
                held.insert(name.clone(), HELD.to_string());
            }
            let Some((image, playlist)) = content.or_else(|| name.is_empty().then_some((None, None))) else {
                continue;
            };
            let o = &profile.outputs[&name];
            let background = o.background.as_deref().and_then(parse_hex_color);
            let background = background.map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"));
            sections.insert(
                name,
                OutputConfig { image, playlist, fit: o.fit, background, preset, ..Default::default() },
            );
        }
        Ok((sections, held))
    }
}
