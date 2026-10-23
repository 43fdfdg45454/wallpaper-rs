use super::block_on;
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid};
use crate::keyring::Stored;
use crate::remote::check_secret;
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_sync::{Ids, Local, Replica, View};

const BINDING: [&str; 3] = ["url", "server", "user"];

impl Daemon {
    fn bound(&self, id: &str) -> Option<View> {
        let (known, _, options) = self.provider(id).ok()?;
        known.secret.as_ref()?;
        let bind: serde_json::Map<String, Value> = options
            .iter()
            .filter(|(k, _)| BINDING.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        let fields = [("provider", json!(id)), ("kind", json!(known.kind)), ("bind", Value::Object(bind))];
        Some(fields.map(|(k, v)| (k.to_owned(), v)).into())
    }

    pub fn secret_views(&self, ids: &Ids, replica: &Replica, local: &mut Local) -> Result<()> {
        let share = self.saved().share.clone();
        let providers: Vec<String> = self.state().config.providers.keys().cloned().collect();
        for (id, mut view) in providers.iter().filter_map(|id| Some((id, self.bound(id)?))) {
            let key = format!("secrets/{}", ids.named("secret", id));
            let base = replica.bases.get(&key).and_then(|(_, doc)| doc.view());
            let synced = base.as_ref().is_some_and(|b| b.contains_key("secret"));
            let same = base.as_ref().is_some_and(|b| b["bind"] == view["bind"] && b["kind"] == view["kind"]);
            let chosen = share.get(id).copied();
            if chosen == Some(false) && synced {
                local.views.insert(key, view);
                continue;
            }
            if chosen != Some(true) && !(synced && same) {
                continue;
            }
            match block_on(self.keyring()?.read(id, false)) {
                Ok(Stored::Secret(secret)) if chosen == Some(true) => {
                    drop(view.insert("secret".into(), json!(secret.as_str())))
                }
                Ok(Stored::Missing) if synced && same => {
                    local.unlinked.insert(key);
                    continue;
                }
                _ => continue,
            }
            local.views.insert(key, view);
        }
        Ok(())
    }

    pub fn apply_secret(&self, view: Option<View>) -> Result<()> {
        let Some(view) = view else { return Ok(()) };
        let id = view.get("provider").and_then(Value::as_str).unwrap_or_default().to_owned();
        let same =
            self.bound(&id).is_some_and(|mine| ["kind", "bind"].iter().all(|f| mine.get(*f) == view.get(*f)));
        let Some(secret) = view.get("secret").and_then(Value::as_str).filter(|_| same) else {
            return self.update(|s| {
                s.shared.remove(&id);
                if s.share.get(&id) == Some(&true) {
                    s.share.remove(&id);
                }
            });
        };
        check_secret(secret)?;
        block_on(self.keyring()?.store(&id, secret))?;
        lock(&self.remote.health).remove(&id);
        tracing::info!(provider = id, "a provider secret arrived from another device");
        self.changes.changed("providers", vec![id.clone()]);
        self.remote.poke.notify_one();
        self.update(|s| {
            s.shared.insert(id);
        })
    }

    pub fn share_secret(self: &Arc<Self>, provider: &str, share: bool) -> Result<()> {
        self.require_ready()?;
        if self.saved().encrypted != Some(true) {
            return Err(failed("secrets sync only with end-to-end encryption"));
        }
        self.bound(provider).ok_or_else(|| invalid(format!("{provider} is no provider with a secret")))?;
        self.update(|s| {
            s.share.insert(provider.to_owned(), share);
            if !share {
                s.shared.remove(provider);
            }
        })?;
        self.account.wake.notify_one();
        Ok(())
    }

    pub fn settle_secrets(&self, replica: &Replica) -> Result<()> {
        let synced: Vec<View> = replica
            .bases
            .iter()
            .filter(|(key, _)| key.starts_with("secrets/"))
            .filter_map(|(_, (_, doc))| doc.view().filter(|v| v.contains_key("secret")))
            .collect();
        let provider = |v: &View| v.get("provider").and_then(Value::as_str).unwrap_or_default().to_owned();
        let here = |v: &View| {
            self.bound(&provider(v)).is_some_and(|m| m["bind"] == v["bind"] && m["kind"] == v["kind"])
        };
        let shared: Vec<String> = synced.iter().filter(|v| here(v)).map(provider).collect();
        let named: Vec<String> = synced.iter().map(provider).collect();
        self.update(|s| {
            s.share.retain(|id, share| *share || named.contains(id));
            s.shared = shared.into_iter().filter(|id| s.share.get(id) != Some(&false)).collect();
        })
    }
}
