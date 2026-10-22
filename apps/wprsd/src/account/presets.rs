use super::local::DEVICE_KEYS;
use super::objects::{Links, keep_name, renamed, text};
use super::views::{Kept, Local, references};
use crate::daemon::Daemon;
use crate::error::Result;
use serde_json::{Value, json};
use wprs_library::Profile;
use wprs_sync::View;

pub fn synced_options(options: &Value, here: Option<&Value>) -> Value {
    let Some(options) = options.as_object() else { return options.clone() };
    let device = |(key, _): &(&String, &Value)| DEVICE_KEYS.contains(&key.as_str());
    let own = here.and_then(Value::as_object).into_iter().flatten().filter(device);
    let shared = options.iter().filter(|option| !device(option));
    Value::Object(shared.chain(own).map(|(key, value)| (key.clone(), value.clone())).collect())
}

impl Daemon {
    pub fn apply_preset(
        &self,
        local: &Local,
        key: &str,
        view: Option<View>,
        links: &mut Links,
        kept: &mut Kept,
    ) -> Result<()> {
        let uid = self.linked_uid(links, key, |lib, uid| lib.preset(uid).is_ok());
        let Some(view) = view else {
            links.remove(key);
            return uid.map_or(Ok(()), |uid| self.delete_preset(&uid));
        };
        let item = match view.get("item").and_then(Value::as_str) {
            Some(item) => match local.items.get(item).and_then(|uids| uids.first()) {
                Some(uid) => Some(uid.clone()),
                None => return Ok(()),
            },
            None => None,
        };
        let (name, value) = (text(&view, "name", "Preset"), view.get("value").cloned().unwrap_or(json!({})));
        let uid = self.store.write(|lib| match uid {
            Some(uid) => lib.update_preset(&uid, &value).map(|()| uid),
            None => renamed(&name, |name| lib.create_preset(item.as_deref(), name, &value)),
        })?;
        let shown = self.store.read(|lib| lib.preset(&uid))?.name;
        keep_name(kept, key, &shown, &name);
        links.insert(key.to_owned(), uid.clone());
        self.changes.changed("profiles", vec![uid]);
        Ok(())
    }

    pub fn apply_profile(
        &self,
        local: &Local,
        key: &str,
        view: Option<View>,
        links: &mut Links,
        kept: &mut Kept,
    ) -> Result<()> {
        let uid = self.linked_uid(links, key, |lib, uid| lib.profile(uid).is_ok());
        let Some(view) = view else {
            links.remove(key);
            return uid.map_or(Ok(()), |uid| self.delete_profile(&uid));
        };
        let here = uid.as_ref().and_then(|uid| self.store.read(|lib| lib.profile(uid)).ok());
        let back = |field: &str, key: &str| match field {
            "item" => local.items.get(key)?.first().cloned(),
            _ => links.get(key).cloned(),
        };
        let outputs = references(view.get("outputs").unwrap_or(&json!({})), None, &back, &|_, _| true);
        let field = |name: &str| view.get(name).cloned().unwrap_or(json!({}));
        let (name, priority) =
            (text(&view, "name", "Profile"), view.get("priority").and_then(Value::as_i64).unwrap_or(0));
        let profile = Profile {
            uid: String::new(),
            name,
            outputs,
            options: synced_options(&field("options"), here.as_ref().map(|p| &p.options)),
            auto: field("auto"),
            priority: priority as i32,
        };
        let uid = self.store.write(|lib| match uid {
            Some(uid) => {
                let mut named = Profile { uid: uid.clone(), ..profile.clone() };
                if lib.update_profile(&named).is_err() {
                    named.name = lib.profile(&uid)?.name;
                    lib.update_profile(&named)?;
                }
                Ok(uid)
            }
            None => renamed(&profile.name, |name| {
                lib.create_profile(name, &profile.outputs, &profile.options, &profile.auto, profile.priority)
            }),
        })?;
        let shown = self.store.read(|lib| lib.profile(&uid))?.name;
        keep_name(kept, key, &shown, &profile.name);
        links.insert(key.to_owned(), uid.clone());
        self.changes.changed("profiles", vec![uid]);
        Ok(())
    }
}
