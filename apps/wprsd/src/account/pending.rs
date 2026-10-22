use super::cycle::rules;
use crate::daemon::Daemon;
use crate::error::Result;
use std::collections::BTreeSet;
use std::sync::Arc;
use wprs_sync::{Hlc, step};

impl Daemon {
    fn local_edits(&self) -> Result<bool> {
        let key = self.secrets(false)?.sync_key()?;
        let (replica, mut links, present) = {
            let s = self.saved();
            (s.replica.clone(), s.links.clone(), s.present.clone())
        };
        let local = self.snapshot(&key.ids(), &replica, &mut links, &present)?;
        let keys: BTreeSet<&String> = replica.bases.keys().chain(local.views.keys()).collect();
        let edited = keys.into_iter().any(|key| {
            let rules = rules(key.split('/').next().unwrap_or_default());
            let base = replica.bases.get(key);
            step(base, None, local.views.get(key), rules, &Hlc::default()).push.is_some()
        });
        Ok(edited || !local.fresh.is_empty())
    }

    pub async fn worth_syncing(self: &Arc<Self>) -> bool {
        let daemon = self.clone();
        let edited = tokio::task::spawn_blocking(move || daemon.local_edits().unwrap_or(true));
        edited.await.unwrap_or(true)
    }
}
