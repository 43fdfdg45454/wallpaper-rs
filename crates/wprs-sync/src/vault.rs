use crate::{DeviceKey, Doc, Error, Lock, Replica, Result, SyncKey};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use zeroize::{Zeroize, Zeroizing};

#[derive(Default, Serialize, Deserialize)]
pub struct Secrets {
    pub token: String,
    pub key: String,
    pub device: String,
    #[serde(default)]
    pub epoch: Option<i64>,
    #[serde(default)]
    pub next: String,
    #[serde(default)]
    pub lock: String,
    #[serde(default)]
    pub records: BTreeMap<String, (i64, Doc)>,
    #[serde(default)]
    pub held: BTreeMap<String, (i64, Doc)>,
    #[serde(default)]
    pub handover: Option<(i64, String, BTreeSet<String>)>,
}

impl Drop for Secrets {
    fn drop(&mut self) {
        let bound = self.handover.as_mut().map(|(_, bound, _)| bound);
        [&mut self.token, &mut self.key, &mut self.device, &mut self.next, &mut self.lock]
            .into_iter()
            .chain(bound)
            .for_each(Zeroize::zeroize);
    }
}

const BROKEN: Error = Error::Crypto("the stored server secrets are broken; remove the server and link again");

pub fn encode(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

fn decode(text: &str) -> Result<Zeroizing<Vec<u8>>> {
    Ok(Zeroizing::new(B64.decode(text).map_err(|_| BROKEN)?))
}

impl Secrets {
    pub fn sync_key(&self) -> Result<SyncKey> {
        SyncKey::from_bytes(&decode(&self.key)?)
            .map_err(|_| Error::Crypto("this device does not hold the sync key yet"))
    }

    pub fn next_key(&self) -> Result<Option<SyncKey>> {
        Ok(SyncKey::from_bytes(&decode(&self.next)?).ok())
    }

    pub fn set_key(&mut self, key: &SyncKey, epoch: i64) {
        let names = |key: &SyncKey| key.ids().named("names", "");
        if self.sync_key().is_ok_and(|old| names(&old) != names(key)) {
            (self.records, self.held) = Default::default();
        }
        (self.key, self.epoch, self.next) = (encode(&key.bytes()), Some(epoch), String::new());
    }

    pub fn escrow_lock(&self) -> Option<Lock> {
        Lock::from_bytes(&decode(&self.lock).ok()?)
    }

    pub fn join(&self, replica: &mut Replica) {
        replica.bases.extend(self.records.clone());
        replica.held.extend(self.held.clone());
    }

    pub fn split(&mut self, replica: &mut Replica) -> bool {
        let take = |map: &mut BTreeMap<String, (i64, Doc)>| {
            let (mine, rest) =
                std::mem::take(map).into_iter().partition(|(key, _)| key.starts_with("secrets/"));
            *map = rest;
            mine
        };
        let (records, held) = (take(&mut replica.bases), take(&mut replica.held));
        let changed = (&records, &held) != (&self.records, &self.held);
        (self.records, self.held) = (records, held);
        changed
    }

    pub fn device_key(&self) -> Result<DeviceKey> {
        DeviceKey::from_bytes(&decode(&self.device)?).map_err(|_| BROKEN)
    }
}
