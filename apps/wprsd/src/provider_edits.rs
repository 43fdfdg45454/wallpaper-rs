use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, not_found};
use crate::kinds::{effective, kind_of, table};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::model::JobKind;

impl Daemon {
    pub async fn set_provider(self: &Arc<Self>, id: String, key: String, value: String) -> Result<()> {
        let (known, mut given, _) = self.provider(&id)?;
        given.insert(key.clone(), value.trim().to_owned());
        effective(&known, &given)?;
        if known.secret.is_some() && ["url", "server", "user"].contains(&key.as_str()) {
            self.keyring()?.delete(&id).await?;
        }
        let entry = table(&known, &given);
        self.run(move |d| {
            d.update_config(|c| {
                c.providers.insert(id.clone(), entry);
                Ok(())
            })?;
            tracing::info!(provider = id, key, "provider option changed");
            d.changes.changed("providers", vec![id]);
            d.remote.poke.notify_one();
            Ok::<_, Error>(())
        })
        .await
    }

    pub async fn remove_provider(self: &Arc<Self>, id: String, purge: bool) -> Result<()> {
        let value = self.state().config.providers.get(&id).cloned();
        let value = value.ok_or_else(|| not_found(format!("provider {id:?} not found")))?;
        self.library_ready()?;
        let kind = value.get("kind").and_then(toml::Value::as_str);
        if self.has_secret(&id, kind).await? {
            self.keyring()?.delete(&id).await?;
        } else if kind.and_then(|k| kind_of(k).ok()?.secret).is_some()
            && !self.keyring()?.reachable(&id).await
        {
            self.orphan(&id, true);
        }
        self.jobs.find(JobKind::ProviderSync, &id).into_iter().try_for_each(|job| self.cancel_job(job))?;
        self.run(move |d| d.drop_provider(&id, purge)).await
    }

    fn drop_provider(&self, id: &str, purge: bool) -> Result<()> {
        if let Some(source) = self.provider_source(id) {
            let removed = self.store.write(|lib| lib.remove_source(&source.uid))?;
            self.forget_files(&removed);
            self.remember_credits(&source.path, &BTreeMap::new());
        }
        self.update_config(|c| {
            c.providers.remove(id);
            Ok(())
        })?;
        match purge {
            true => std::fs::remove_dir_all(self.provider_dir(id)).ok(),
            false => std::fs::remove_file(self.provider_dir(id).join(crate::remote::CREDITS)).ok(),
        };
        lock(&self.remote.health).remove(id);
        tracing::info!(provider = id, "provider removed");
        self.library_changed(Vec::new());
        self.changes.changed("providers", vec![id.to_owned()]);
        Ok(())
    }
}
