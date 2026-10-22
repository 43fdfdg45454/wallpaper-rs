use super::{KINDS, block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use wprs_core::model::JobKind;

impl Daemon {
    pub fn approve(self: &Arc<Self>, code: &str) -> Result<u64> {
        let letters: String = code.chars().filter(char::is_ascii_alphanumeric).collect();
        if letters.len() != 8 {
            return Err(invalid("give the 8 letters shown on the new device"));
        }
        self.linked()?;
        Ok(self.start_job(JobKind::Account, &format!("approve {letters}"), move |d, _| {
            let (api, _) = d.account_api(true)?;
            let body = json!({"user_code": letters, "approve": true});
            block_on(api.post("/api/v1/auth/device/approve", body)).map_err(sync_error)
        }))
    }

    pub fn set_sync_paused(&self, paused: bool) -> Result<()> {
        self.linked()?;
        self.update(|s| s.paused = paused)?;
        self.account.restart.notify_one();
        if !paused {
            self.account.wake.notify_one();
        }
        Ok(())
    }

    pub fn remove_server(&self, wipe: bool) -> Result<()> {
        self.server_url()?;
        for job in KINDS.into_iter().flat_map(|kind| self.jobs_of(kind)) {
            self.cancel_job(job)?;
        }
        let device = self.saved().device.clone();
        let revoke = device.and_then(|device| Some((self.account_api(false).ok()?.0, device)));
        if self.saved().device.is_some() {
            self.forget_secrets()?;
        }
        if let Some((api, device)) = revoke {
            tokio::runtime::Handle::current().spawn(async move {
                let path = format!("/api/v1/me/devices/{device}");
                let gone = tokio::time::timeout(
                    Duration::from_secs(10),
                    api.call(reqwest::Method::DELETE, &path, None),
                );
                if !matches!(gone.await, Ok(Ok(_))) {
                    tracing::warn!("the server could not be told to forget this computer");
                }
            });
        }
        self.update(|s| s.forget(wipe))?;
        self.account.restart.notify_one();
        tracing::info!("server removed");
        Ok(())
    }

    pub fn require_ready(&self) -> Result<()> {
        self.linked()?;
        let ready = self.saved().ready;
        if ready {
            Ok(())
        } else {
            Err(failed("this computer has no sync key yet; join from a linked device"))
        }
    }
}
