use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, invalid, not_found};
use crate::kinds::kind_of;
use crate::remote::{check_secret, needs_secret};
use std::sync::Arc;
use zeroize::Zeroizing;

impl Daemon {
    pub async fn has_secret(&self, id: &str, kind: Option<&str>) -> Result<bool> {
        let secret = kind.and_then(|kind| kind_of(kind).ok()?.secret).is_some() && !self.orphaned(id);
        match self.keyring().ok().filter(|_| secret) {
            Some(keyring) => keyring.has(id).await,
            None => Ok(false),
        }
    }

    pub async fn drop_orphan(&self, id: &str) -> Result<()> {
        if !self.keyring()?.discard(id).await.unwrap_or(false) {
            self.orphan(id, true);
        }
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
        self.orphan(&id, false);
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
