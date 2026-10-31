use crate::daemon::Daemon;
use crate::error::{Error, Result};
use crate::jobs::now;
use std::sync::Arc;
use wprs_sync::{Ids, Local, Replica};

impl Daemon {
    pub fn forget_synced(&self, removed: Vec<String>) {
        if removed.is_empty() || self.saved().device.is_none() {
            return;
        }
        let at = now() * 1000;
        if let Err(e) = self.update(|s| s.removed.extend(removed.into_iter().map(|r| (r, at)))) {
            tracing::warn!("a removal could not be kept for the other devices: {e}");
        }
    }

    pub fn removals(&self, ids: &Ids, replica: &mut Replica) {
        wprs_sync::removals(ids, &self.saved().removed, replica, now() as u64 * 1000);
    }

    pub fn delete_synced(self: &Arc<Self>, local: &Local, item: &str) -> Result<()> {
        let installed = self.state().installed.clone();
        for uid in local.items.get(item).into_iter().flatten() {
            let found = self.store.read(|lib| lib.item(uid));
            let Ok(found) = found else { continue };
            match installed.iter().find(|i| i.path == found.path) {
                Some(package) => self.uninstall(&package.id, "")?,
                None => match self.remove_items(std::slice::from_ref(uid), false) {
                    Err(Error::Fdo(zbus::fdo::Error::InvalidArgs(why))) => {
                        tracing::info!("a deleted item stays: {why}")
                    }
                    done => done?,
                },
            }
        }
        Ok(())
    }
}
