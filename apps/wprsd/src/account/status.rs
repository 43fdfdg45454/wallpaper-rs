use super::saved::{Peer, Saved};
use crate::daemon::Daemon;
use crate::error::Result;
use crate::items::reply;
use wprs_core::model::{Account, AccountDevice, AccountReply, AccountState, JobKind, MODEL};

impl Saved {
    pub fn label_of(&self, id: &str) -> Option<String> {
        if self.device.as_deref() == Some(id) {
            return Some(self.label.clone());
        }
        let (_, doc) = self.replica.bases.get(&format!("device/{id}"))?;
        doc.view()?.get("name")?.as_str().map(String::from)
    }

    fn state(&self, linking: bool) -> AccountState {
        match () {
            _ if linking => AccountState::Linking,
            _ if self.device.is_none() => AccountState::Unlinked,
            _ if !self.ready => AccountState::NeedsKey,
            _ if self.paused => AccountState::Paused,
            _ if self.last_error.is_some() => AccountState::Error,
            _ => AccountState::Ready,
        }
    }

    pub fn model(&self, linking: bool, job: Option<u64>) -> Option<Account> {
        let device = |p: &Peer| AccountDevice {
            id: p.id.clone(),
            name: p.name.clone(),
            label: self.label_of(&p.id),
            platform: p.platform.clone(),
            created_at: p.created_at,
            seen_at: p.seen_at,
            current: self.device.as_ref() == Some(&p.id),
        };
        (!self.url.is_empty()).then(|| Account {
            url: self.url.clone(),
            name: self.name.clone(),
            repo_id: self.repo_id.clone(),
            handle: self.handle.clone(),
            device: self.device_name.clone(),
            label: self.label.clone(),
            state: self.state(linking),
            encrypted: self.encrypted,
            warning: self.warning.clone(),
            paused: self.paused,
            last_sync: self.last_sync,
            last_error: self.last_error.clone(),
            records: self.replica.bases.len() as u32,
            used_bytes: self.used_bytes,
            quota_bytes: self.quota_bytes,
            devices: self.peers.iter().map(device).collect(),
            job,
        })
    }
}

impl Daemon {
    pub fn account_status(&self) -> Result<String> {
        let serial = self.changes.serial();
        let job = super::KINDS.into_iter().flat_map(|kind| self.jobs_of(kind)).min();
        let linking = !self.jobs.find(JobKind::Link, "link").is_empty();
        let account = self.saved().model(linking, job);
        reply(AccountReply { model: MODEL, serial, account })
    }
}
