use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid, not_found};
use crate::jobs::now;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::hash::{BuildHasher, RandomState};
use wprs_core::dbus::uri_scheme;
use wprs_core::model::{
    ItemDetails, ItemSummary, MODEL, PlaylistRef, Preset, SearchPage, SourcesReply, Tag, TagsReply,
};
use wprs_format::Installed;
use wprs_library::{Item, Library};

pub const MAX_UIDS: usize = 10_000;
const MAX_PAGE: u32 = 500;

pub fn merged<T: DeserializeOwned>(base: impl Serialize, extra: Value) -> Result<T> {
    let mut value = serde_json::to_value(base).map_err(|e| failed(e.to_string()))?;
    if let (Some(object), Value::Object(extra)) = (value.as_object_mut(), extra) {
        object.extend(extra);
    }
    serde_json::from_value(value).map_err(|e| failed(e.to_string()))
}

pub fn reply(value: impl Serialize) -> Result<String> {
    serde_json::to_string(&value).map_err(|e| failed(e.to_string()))
}

pub fn preset(preset: wprs_library::Preset) -> Preset {
    let value = serde_json::from_value(preset.value)
        .unwrap_or(wprs_core::model::PresetValue { fit: None, background: None });
    Preset { uid: preset.uid, item: preset.item, name: preset.name, value }
}

pub fn limited<T>(list: &[T], max: usize) -> Result<()> {
    if list.len() > max { Err(invalid(format!("give at most {max} entries"))) } else { Ok(()) }
}

impl Daemon {
    pub fn installed(&self, item: &Item) -> Option<Installed> {
        self.state().installed.iter().find(|i| i.path == item.path).cloned()
    }

    fn summary(&self, item: Item) -> Result<ItemSummary> {
        let installed = self.installed(&item);
        let removable = installed.is_none() && (item.source.is_none() || !item.path.exists());
        let (imported, thumb) = (item.path.starts_with(self.store.files()), self.thumbs.cached(&item, 256));
        let revoked = installed.and_then(|i| i.revoked);
        merged(
            item,
            json!({"revoked": revoked, "removable": removable, "imported": imported, "thumb": thumb}),
        )
    }

    pub fn list_sources(&self) -> Result<String> {
        let serial = self.changes.serial();
        let sources = self.store.read(|lib| {
            let source = |s: wprs_library::Source| {
                let (items, provider) =
                    (lib.count(&format!("source:{}", s.uid), now())?, self.provider_of(&s.path));
                merged(s, json!({"items": items, "provider": provider}))
            };
            lib.sources()?.into_iter().map(source).collect::<Result<Vec<_>>>()
        })?;
        reply(SourcesReply { model: MODEL, serial, sources })
    }

    pub fn search(&self, query: &str, offset: u32, limit: u32) -> Result<String> {
        if limit > MAX_PAGE {
            return Err(invalid(format!("limit must be at most {MAX_PAGE}")));
        }
        let serial = self.changes.serial();
        let seed = (RandomState::new().hash_one(serial) % 2_147_483_646) as u32 + 1;
        let (query, total, items) = self.store.read(|lib| {
            let query = lib.normalize_query(query, seed)?;
            let total = lib.count(&query, now())?;
            let items = if limit == 0 { Vec::new() } else { lib.search(&query, now(), offset, limit)? };
            Ok::<_, wprs_library::Error>((query, total, items))
        })?;
        let items = items.into_iter().map(|item| self.summary(item)).collect::<Result<_>>()?;
        reply(SearchPage { model: MODEL, serial, query, total, offset, items })
    }

    pub fn get_item(&self, uid: &str) -> Result<String> {
        let (model, serial, credit) = (MODEL, self.changes.serial(), None);
        let found = |lib: &Library| {
            Ok::<_, wprs_library::Error>((lib.item(uid)?, lib.presets(Some(uid))?, lib.item_playlists(uid)?))
        };
        let (item, presets, playlists) = self.store.read(found)?;
        let (thumb, installed) = (self.thumbs.cached(&item, 512), self.installed(&item));
        let revoked =
            installed.as_ref().and_then(|i| i.revoked).map(|r| merged(r, Value::Null)).transpose()?;
        let installed = installed.map(|i| merged(i, json!({"item": uid, "update": null}))).transpose()?;
        let presets = presets.into_iter().map(preset).collect();
        let playlists = playlists.into_iter().map(|p| PlaylistRef { uid: p.uid, name: p.name }).collect();
        let item = merged(item, Value::Null)?;
        reply(ItemDetails { model, serial, item, thumb, presets, playlists, credit, installed, revoked })
    }

    pub fn resolve(&self, refs: &[String]) -> Result<Vec<String>> {
        limited(refs, MAX_UIDS)?;
        let found = |lib: &Library, r: &String| match lib.resolve_item(r) {
            _ if uri_scheme(r).is_some() => Err(invalid(format!("{r} is a URI; give a local path"))),
            Err(wprs_library::Error::NotFound(..)) => Err(not_found(format!("no item matches {r:?}"))),
            Err(wprs_library::Error::Invalid(_, message)) => Err(invalid(message)),
            other => Ok(other?),
        };
        self.store.read(|lib| refs.iter().map(|r| found(lib, r)).collect::<Result<Vec<_>>>())
    }

    pub fn list_tags(&self) -> Result<String> {
        let serial = self.changes.serial();
        let tags = self.store.read(|lib| lib.tags())?.into_iter().map(|(name, count)| Tag { name, count });
        reply(TagsReply { model: MODEL, serial, tags: tags.collect() })
    }

    pub fn edit_items(
        &self,
        uids: &[String],
        change: impl FnOnce(&mut Library, &[&str]) -> wprs_library::Result<()>,
    ) -> Result<()> {
        limited(uids, MAX_UIDS)?;
        let refs: Vec<&str> = uids.iter().map(String::as_str).collect();
        self.store.write(|lib| change(lib, &refs))?;
        self.changes.changed("library", uids.to_vec());
        Ok(())
    }

    pub fn remove_items(&self, uids: &[String], delete_files: bool) -> Result<()> {
        limited(uids, MAX_UIDS)?;
        let (files, refs) = (self.store.files(), uids.iter().map(String::as_str).collect::<Vec<_>>());
        let installed: Vec<_> = self.state().installed.iter().map(|i| i.path.clone()).collect();
        let refused = |item: &Item| match () {
            () if installed.contains(&item.path) => {
                Some("is installed from a repository; use Repos1.Uninstall")
            }
            () if item.source.is_some() && item.path.exists() => {
                Some("is in a source; remove the file or the folder")
            }
            () if delete_files && !item.path.starts_with(&files) => {
                Some("is not an imported copy; refusing to delete files outside the imported-files folder")
            }
            () => None,
        };
        let removed = self.store.write(|lib| {
            for item in refs.iter().map(|uid| lib.item(uid)).collect::<Result<Vec<_>, _>>()? {
                if let Some(reason) = refused(&item) {
                    return Err(invalid(format!("item {} {reason}", item.uid)));
                }
            }
            Ok(lib.remove_items(&refs)?)
        })?;
        self.forget_files(&removed);
        removed.iter().filter(|_| delete_files).for_each(|item| drop(std::fs::remove_file(&item.path)));
        self.library_changed(uids.to_vec());
        Ok(())
    }

    pub fn forget_files(&self, items: &[Item]) {
        for item in items {
            self.thumbs.remove(item);
            drop(std::fs::remove_file(self.blob(&item.path, item.mtime, item.size)));
        }
    }

    pub fn library_changed(&self, ids: Vec<String>) {
        self.changes.changed("library", ids);
        self.refresh_items();
    }
}
