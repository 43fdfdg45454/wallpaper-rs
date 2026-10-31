use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::Result;
use crate::keyring::Keyring;
use futures_util::StreamExt;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use zbus::message::Type;
use zbus::{MatchRule, MessageStream};

const FILE: &str = "orphan-secrets.json";
const SERVICE: &str = "org.freedesktop.secrets";

#[derive(Default)]
pub struct Orphans {
    ids: Mutex<BTreeSet<String>>,
    pub wake: Notify,
    pub writing: tokio::sync::Mutex<()>,
}

impl Orphans {
    pub fn load(&self, data: &Path) {
        let bytes = fs::read(data.join(FILE)).unwrap_or_default();
        let found: BTreeSet<String> = serde_json::from_slice(&bytes).unwrap_or_default();
        lock(&self.ids).extend(found);
    }

    pub fn has(&self, id: &str) -> bool {
        lock(&self.ids).contains(id)
    }

    pub fn any(&self) -> bool {
        !lock(&self.ids).is_empty()
    }

    pub fn mark(&self, data: &Path, id: &str, pending: bool) -> bool {
        let ids = {
            let mut ids = lock(&self.ids);
            let changed = if pending { ids.insert(id.to_owned()) } else { ids.remove(id) };
            changed.then(|| ids.clone())
        };
        let Some(ids) = ids else { return false };
        if crate::reveals::save(data, FILE, &ids).is_err() {
            tracing::warn!(provider = id, "the list of orphaned secrets could not be saved");
        }
        if pending {
            tracing::warn!(provider = id, "an older secret stays in the locked keyring until it is unlocked");
            self.wake.notify_one();
        }
        true
    }

    pub async fn replace(
        &self,
        keyring: &Keyring,
        data: &Path,
        id: &str,
        secret: &str,
    ) -> (Result<()>, bool) {
        let _writing = self.writing.lock().await;
        let was = self.has(id);
        let cleared = self.mark(data, id, false);
        let stored = keyring.store(id, secret).await;
        let back = stored.is_err() && was && self.mark(data, id, true);
        (stored, cleared != back)
    }

    pub async fn drop_one(&self, keyring: &Keyring, data: &Path, id: &str) -> bool {
        let _writing = self.writing.lock().await;
        !keyring.discard(id).await.unwrap_or(false) && self.mark(data, id, true)
    }

    pub async fn sweep(&self, keyring: &Keyring, data: &Path) -> Vec<String> {
        let ids: Vec<String> = lock(&self.ids).iter().cloned().collect();
        let mut cleared = Vec::new();
        for id in ids {
            let _writing = self.writing.lock().await;
            if self.has(&id) && keyring.discard(&id).await.unwrap_or(false) {
                tracing::info!(provider = id, "the orphaned secret was deleted from the unlocked keyring");
                self.mark(data, &id, false);
                cleared.push(id);
            }
        }
        cleared
    }
}

impl Daemon {
    pub fn load_orphans(&self) {
        self.remote.orphans.load(&self.store.data);
    }

    pub fn orphaned(&self, id: &str) -> bool {
        self.remote.orphans.has(id)
    }

    pub fn orphan(&self, id: &str, pending: bool) {
        if self.remote.orphans.mark(&self.store.data, id, pending) {
            self.changes.changed("providers", vec![id.to_owned()]);
        }
    }

    pub async fn replace_secret(&self, id: &str, secret: &str) -> Result<()> {
        let keyring = self.keyring()?;
        let (stored, changed) = self.remote.orphans.replace(keyring, &self.store.data, id, secret).await;
        if changed {
            self.changes.changed("providers", vec![id.to_owned()]);
        }
        stored
    }

    pub async fn drop_orphan(&self, id: &str) -> Result<()> {
        let keyring = self.keyring()?;
        if self.remote.orphans.drop_one(keyring, &self.store.data, id).await {
            self.changes.changed("providers", vec![id.to_owned()]);
        }
        Ok(())
    }

    async fn sweep(&self) {
        let Ok(keyring) = self.keyring() else { return };
        let cleared = self.remote.orphans.sweep(keyring, &self.store.data).await;
        if !cleared.is_empty() {
            self.changes.changed("providers", cleared);
        }
    }
}

pub async fn watch(daemon: Arc<Daemon>, bus: zbus::Connection) {
    let owner = MatchRule::builder().msg_type(Type::Signal).sender("org.freedesktop.DBus");
    let owner = owner.interface("org.freedesktop.DBus").member("NameOwnerChanged").arg(0, SERVICE).build();
    let keyring = MatchRule::builder().msg_type(Type::Signal).sender(SERVICE).build();
    let stream = |rule: zbus::Result<MatchRule<'static>>| {
        let bus = bus.clone();
        async move { MessageStream::for_match_rule(rule?, &bus, None).await }
    };
    let (mut owner, mut keyring) = (stream(owner).await.ok(), stream(keyring).await.ok());
    loop {
        if daemon.remote.orphans.any() {
            daemon.sweep().await;
        }
        let wake = daemon.remote.orphans.wake.notified();
        tokio::select! {
            Some(_) = async { owner.as_mut()?.next().await } => {}
            Some(_) = async { keyring.as_mut()?.next().await } => {}
            () = wake => {}
        }
    }
}

#[cfg(test)]
mod tests;
