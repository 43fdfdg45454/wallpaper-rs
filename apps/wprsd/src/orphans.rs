use crate::compositor::lock;
use crate::daemon::Daemon;
use futures_util::StreamExt;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use zbus::message::Type;
use zbus::{MatchRule, MessageStream};

const FILE: &str = "orphan-secrets.json";
const SERVICE: &str = "org.freedesktop.secrets";

impl Daemon {
    pub fn load_orphans(&self) {
        let bytes = fs::read(self.store.data.join(FILE)).unwrap_or_default();
        let found: BTreeSet<String> = serde_json::from_slice(&bytes).unwrap_or_default();
        lock(&self.remote.orphans).extend(found);
    }

    pub fn orphaned(&self, id: &str) -> bool {
        lock(&self.remote.orphans).contains(id)
    }

    pub fn orphan(&self, id: &str, pending: bool) {
        let ids = {
            let mut ids = lock(&self.remote.orphans);
            let changed = if pending { ids.insert(id.to_owned()) } else { ids.remove(id) };
            changed.then(|| ids.clone())
        };
        let Some(ids) = ids else { return };
        let saved =
            tempfile::Builder::new().prefix(".tmp-").tempfile_in(&self.store.data).and_then(|mut f| {
                f.write_all(&serde_json::to_vec(&ids).unwrap_or_default())?;
                f.persist(self.store.data.join(FILE)).map(drop).map_err(|e| e.error)
            });
        if saved.is_err() {
            tracing::warn!(provider = id, "the list of orphaned secrets could not be saved");
        }
        if pending {
            tracing::warn!(provider = id, "an older secret stays in the locked keyring until it is unlocked");
            self.remote.orphan_wake.notify_one();
        }
        self.changes.changed("providers", vec![id.to_owned()]);
    }

    async fn sweep(&self) {
        let ids: Vec<String> = lock(&self.remote.orphans).iter().cloned().collect();
        let Ok(keyring) = self.keyring() else { return };
        for id in ids {
            if keyring.discard(&id).await.unwrap_or(false) {
                tracing::info!(provider = id, "the orphaned secret was deleted from the unlocked keyring");
                self.orphan(&id, false);
            }
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
        if !lock(&daemon.remote.orphans).is_empty() {
            daemon.sweep().await;
        }
        let wake = daemon.remote.orphan_wake.notified();
        tokio::select! {
            Some(_) = async { owner.as_mut()?.next().await } => {}
            Some(_) = async { keyring.as_mut()?.next().await } => {}
            () = wake => {}
        }
    }
}
