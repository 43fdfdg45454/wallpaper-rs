use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, invalid, not_found};
use crate::items::reply;
use crate::jobs::now;
use crate::kinds::{Options, effective, entry, hosts, kind_of, table};
use crate::remote::{check_secret, claim, last_sync, needs_secret};
use crate::store::private_dir;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use wprs_core::dbus::error::{EXISTS, NEEDS_SECRET};
use wprs_core::model::{JobKind, MODEL, Provider, ProviderState, ProvidersReply};
use zeroize::Zeroizing;

const LIST_WAIT: Duration = Duration::from_secs(1);

fn trimmed(options: HashMap<String, String>) -> Options {
    options.into_iter().map(|(key, value)| (key, value.trim().to_owned())).collect()
}

impl Daemon {
    pub async fn has_secret(&self, id: &str, kind: Option<&str>) -> Result<bool> {
        let secret = kind.and_then(|kind| kind_of(kind).ok()?.secret).is_some();
        match self.keyring().ok().filter(|_| secret) {
            Some(keyring) => keyring.has(id).await,
            None => Ok(false),
        }
    }

    pub async fn list_providers(self: &Arc<Self>) -> Result<String> {
        let serial = self.changes.serial();
        let entries: Vec<(String, toml::Value)> = self.state().config.providers.clone().into_iter().collect();
        let check = |(id, value): &(String, toml::Value)| {
            let kind = value.get("kind").and_then(toml::Value::as_str).map(String::from);
            let (d, id) = (self.clone(), id.clone());
            tokio::spawn(async move { d.has_secret(&id, kind.as_deref()).await })
        };
        let (checks, until) = (entries.iter().map(check).collect::<Vec<_>>(), Instant::now() + LIST_WAIT);
        let mut providers = Vec::new();
        for ((id, value), mut check) in entries.into_iter().zip(checks) {
            let known = || self.keyring().ok().and_then(|k| k.known(&id));
            let has_secret = match tokio::time::timeout_at(until, &mut check).await {
                Ok(Ok(Ok(found))) => Some(found),
                Ok(_) => known(),
                Err(_) => {
                    let (d, shown, id) = (self.clone(), known(), id.clone());
                    tokio::spawn(async move {
                        if check.await.is_ok_and(|found| found.ok() != shown) {
                            d.changes.changed("providers", vec![id]);
                        }
                    });
                    shown
                }
            };
            providers.push(self.run(move |d| Ok::<_, Error>(d.describe(&id, &value, has_secret))).await?);
        }
        reply(ProvidersReply { model: MODEL, serial, providers })
    }

    fn describe(&self, id: &str, value: &toml::Value, found: Option<bool>) -> Provider {
        let has_secret = found.unwrap_or_default();
        let kind = value.get("kind").and_then(toml::Value::as_str).unwrap_or_default().to_owned();
        let source = self.provider_source(id);
        let query = |uid: &str| format!("source:{uid} missing:no");
        let items =
            source.as_ref().and_then(|s| self.store.read(|lib| lib.count(&query(&s.uid), now())).ok());
        let files = std::fs::read_dir(self.provider_dir(id)).into_iter().flatten().flatten();
        let cache_bytes =
            files.filter_map(|f| f.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum();
        let job = self
            .jobs
            .find(JobKind::ProviderSync, id)
            .into_iter()
            .chain(self.jobs.find(JobKind::ProviderSync, ""));
        let job = job.min();
        let (last_error, rejected, locked) = lock(&self.remote.health)
            .get(id)
            .map_or((None, false, false), |h| (h.last_error.clone(), h.rejected, h.locked));
        let mut provider = Provider {
            id: id.into(),
            kind,
            options: BTreeMap::new(),
            source: source.map(|s| s.uid),
            state: ProviderState::Error,
            has_secret,
            hosts: Vec::new(),
            items: items.unwrap_or_default(),
            cache_bytes,
            last_sync: last_sync(&self.provider_dir(id)),
            next_sync: None,
            last_error,
            job,
        };
        let (known, _, options) = match entry(value) {
            Ok(parsed) => parsed,
            Err(message) => return Provider { last_error: Some(message), ..provider },
        };
        let required = known.secret.as_ref().is_some_and(|s| s.required);
        let (missing, silent) = (required && found == Some(false), required && found.is_none());
        provider.state = match () {
            () if job.is_some() => ProviderState::Syncing,
            () if missing || rejected => ProviderState::NeedsSecret,
            () if locked || silent => ProviderState::Locked,
            () if provider.last_error.is_some() => ProviderState::Error,
            () => ProviderState::Idle,
        };
        let needs = provider.state == ProviderState::NeedsSecret;
        provider.next_sync = self.next_sync(id, &options).filter(|_| !needs);
        Provider { hosts: hosts(&known, &options), options, ..provider }
    }

    pub async fn add_provider(
        self: &Arc<Self>,
        id: String,
        kind: String,
        options: HashMap<String, String>,
        secret: Zeroizing<String>,
    ) -> Result<String> {
        crate::kinds::check_id(&id)?;
        let (known, given) = (kind_of(&kind)?, trimmed(options));
        effective(&known, &given)?;
        if !secret.is_empty() {
            known.secret.as_ref().ok_or_else(|| invalid(format!("{kind} takes no secret")))?;
            check_secret(&secret)?;
        }
        let exists = || Error::Named(EXISTS, format!("provider {id:?} already exists"));
        let _adding = claim(&self.remote.adding, &id).ok_or_else(exists)?;
        if self.state().config.providers.contains_key(&id) {
            return Err(exists());
        }
        self.library_ready()?;
        if !secret.is_empty() {
            self.keyring()?.store(&id, &secret).await?;
        } else if known.secret.is_some() {
            self.drop_orphan(&id).await?;
        }
        let (owned, entry) = (id.clone(), table(&known, &given));
        let added = self.run(move |d| d.create_provider(&owned, entry)).await;
        if added.is_err() && !secret.is_empty() {
            self.keyring()?.delete(&id).await.ok();
        }
        let source = added?;
        tracing::info!(provider = id, kind, "provider added");
        self.remote.poke.notify_one();
        Ok(source)
    }

    async fn drop_orphan(&self, id: &str) -> Result<()> {
        match self.keyring() {
            Ok(keyring) if !keyring.discard(id).await? => Err(Error::Named(
                NEEDS_SECRET,
                format!(
                    "the locked keyring holds an older secret for {id}; give the secret now to replace it, \
                     or unlock the keyring and add the provider again to delete it"
                ),
            )),
            _ => Ok(()),
        }
    }

    fn create_provider(&self, id: &str, entry: toml::Value) -> Result<String> {
        let dir = self.provider_dir(id);
        private_dir(&dir)?;
        let source = self.store.write(|lib| lib.add_folder(&dir, false))?;
        let saved = self.update_config(|c| {
            c.providers.insert(id.to_owned(), entry);
            Ok(())
        });
        if let Err(e) = saved {
            self.store.write(|lib| lib.remove_source(&source)).ok();
            return Err(e.into());
        }
        lock(&self.remote.health).remove(id);
        self.library_changed(Vec::new());
        self.changes.changed("providers", vec![id.to_owned()]);
        Ok(source)
    }

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
        if self.has_secret(&id, value.get("kind").and_then(toml::Value::as_str)).await? {
            self.keyring()?.delete(&id).await?;
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
        if purge {
            std::fs::remove_dir_all(self.provider_dir(id)).ok();
        }
        lock(&self.remote.health).remove(id);
        tracing::info!(provider = id, "provider removed");
        self.library_changed(Vec::new());
        self.changes.changed("providers", vec![id.to_owned()]);
        Ok(())
    }

    pub async fn set_secret(self: &Arc<Self>, id: String, secret: Zeroizing<String>) -> Result<()> {
        let (known, ..) = self.provider(&id)?;
        known.secret.ok_or_else(|| invalid(format!("{} takes no secret", known.kind)))?;
        if secret.is_empty() {
            return Err(invalid("the secret is empty"));
        }
        check_secret(&secret)?;
        self.keyring()?.store(&id, &secret).await?;
        lock(&self.remote.health).remove(&id);
        tracing::info!(provider = id, "provider secret stored");
        self.changes.changed("providers", vec![id]);
        self.remote.poke.notify_one();
        Ok(())
    }

    pub async fn forget_secret(self: &Arc<Self>, id: String) -> Result<()> {
        let value = self.state().config.providers.get(&id).cloned();
        let value = value.ok_or_else(|| not_found(format!("provider {id:?} not found")))?;
        let secret = value.get("kind").and_then(toml::Value::as_str).and_then(|k| kind_of(k).ok()?.secret);
        if secret.is_some() {
            self.keyring()?.delete(&id).await?;
        }
        self.changes.changed("providers", vec![id]);
        Ok(())
    }

    pub async fn sync_providers(self: &Arc<Self>, id: String) -> Result<u64> {
        self.library_ready()?;
        if !id.is_empty() {
            let (known, ..) = self.provider(&id)?;
            let required = known.secret.is_some_and(|s| s.required);
            if required && !self.has_secret(&id, Some(&known.kind)).await? {
                return Err(needs_secret(&id));
            }
        }
        Ok(self.sync_job(&id, true))
    }
}
