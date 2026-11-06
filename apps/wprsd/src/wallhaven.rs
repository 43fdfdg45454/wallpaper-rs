use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid};
use crate::items::reply;
use crate::keyring::Stored;
use crate::sync::failure;
use std::sync::Arc;
use wprs_core::model::{CollectionsReply, MODEL, WallhavenCollection};
use wprs_providers::wallhaven::{Search, new_seed, option_hint};
use wprs_providers::{Secret, Wallhaven};

impl Daemon {
    pub fn seed_of(&self, id: &str) -> String {
        lock(&self.remote.seeds).entry(id.to_owned()).or_insert_with(new_seed).clone()
    }

    fn check_wallhaven(&self, id: &str) -> Result<crate::kinds::Options> {
        let (known, _, options) = self.provider(id)?;
        let only = || invalid(format!("{id} is not a wallhaven provider"));
        (known.kind == "wallhaven").then_some(options).ok_or_else(only)
    }

    pub async fn reseed(self: &Arc<Self>, id: String) -> Result<u64> {
        self.check_wallhaven(&id)?;
        lock(&self.remote.seeds).insert(id.clone(), new_seed());
        self.sync_providers(id).await
    }

    pub async fn wallhaven_collections(self: &Arc<Self>, id: String, owner: String) -> Result<String> {
        let options = self.check_wallhaven(&id)?;
        let owner = owner.trim();
        if let Some(hint) = option_hint("uploader", owner) {
            return Err(invalid(format!("invalid owner: expected {hint}")));
        }
        let key = match self.keyring()?.read(&id, true).await? {
            Stored::Secret(key) => Some(key),
            _ => None,
        };
        let search = Search::from_options(|name| options.get(name).cloned().unwrap_or_default());
        let api_key = key.as_ref().map(|key| Secret::from(key.as_str()));
        let wallhaven = Wallhaven { search, api_key, ..Wallhaven::new().map_err(|e| failed(e.to_string()))? };
        let found = wallhaven.collections(Some(owner).filter(|owner| !owner.is_empty())).await;
        let found = found.map_err(|e| failure(e, key.is_some()))?;
        let collection = |c: wprs_providers::wallhaven::Collection| WallhavenCollection {
            id: c.id,
            label: c.label,
            count: c.count,
            public: c.public,
        };
        let collections = found.into_iter().map(collection).collect();
        reply(CollectionsReply { model: MODEL, serial: self.changes.serial(), collections })
    }
}
