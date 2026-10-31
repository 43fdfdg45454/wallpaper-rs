use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, invalid};
use crate::items::reply;
use crate::jobs::now;
use crate::kinds::{Options, effective, entry, hosts, kind_of, table};
use crate::remote::{check_secret, claim, last_sync};
use crate::store::private_dir;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use wprs_core::dbus::error::EXISTS;
use wprs_core::model::{JobKind, MODEL, Provider, ProviderState, ProvidersReply};
use zeroize::Zeroizing;

const LIST_WAIT: Duration = Duration::from_secs(1);

fn trimmed(options: HashMap<String, String>) -> Options {
    options.into_iter().map(|(key, value)| (key, value.trim().to_owned())).collect()
}

impl Daemon {
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
            orphan_secret: self.orphaned(id),
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
            self.replace_secret(&id, &secret).await?;
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
}
