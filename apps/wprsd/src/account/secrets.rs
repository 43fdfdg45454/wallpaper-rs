use super::block_on;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed};
use crate::keyring::{Keyring, Stored};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use wprs_core::dbus::error::KEYRING_UNAVAILABLE;
use wprs_sync::{Api, DeviceKey, SyncKey};
use zeroize::{Zeroize, Zeroizing};

const ID: &str = "server";

#[derive(Default, Serialize, Deserialize)]
pub struct Secrets {
    pub token: String,
    pub key: String,
    pub device: String,
}

impl Drop for Secrets {
    fn drop(&mut self) {
        [&mut self.token, &mut self.key, &mut self.device].into_iter().for_each(Zeroize::zeroize);
    }
}

fn broken() -> Error {
    failed("the server secrets in the keyring are broken; remove the server and link again")
}

impl Secrets {
    pub fn sync_key(&self) -> Result<SyncKey> {
        let bytes = Zeroizing::new(B64.decode(&self.key).map_err(|_| broken())?);
        SyncKey::from_bytes(&bytes).map_err(|_| failed("this computer does not hold the sync key yet"))
    }

    pub fn device_key(&self) -> Result<DeviceKey> {
        DeviceKey::from_bytes(&Zeroizing::new(B64.decode(&self.device).map_err(|_| broken())?))
            .map_err(|_| broken())
    }
}

pub fn encode(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

impl Daemon {
    fn server_keyring(&self) -> Result<&Keyring> {
        self.account.keyring.get().ok_or_else(|| failed("the keyring is not ready yet"))
    }

    pub async fn read_secrets(&self, prompt: bool) -> Result<Secrets> {
        match self.server_keyring()?.read(ID, prompt).await? {
            Stored::Secret(text) => serde_json::from_str(&text).map_err(|_| broken()),
            Stored::Locked => {
                Err(Error::Named(KEYRING_UNAVAILABLE, "unlock the keyring and try again".into()))
            }
            Stored::Missing => {
                Err(failed("the keyring holds no token for this server; link this computer again"))
            }
        }
    }

    pub fn secrets(&self, prompt: bool) -> Result<Secrets> {
        block_on(self.read_secrets(prompt))
    }

    pub fn store_secrets(&self, secrets: &Secrets) -> Result<()> {
        let text = Zeroizing::new(serde_json::to_string(secrets).map_err(|e| failed(e.to_string()))?);
        block_on(self.server_keyring()?.store(ID, &text))
    }

    pub fn forget_secrets(&self) -> Result<()> {
        block_on(self.server_keyring()?.delete(ID))
    }

    pub async fn events(&self) -> Result<reqwest::Response> {
        let secrets = self.read_secrets(false).await?;
        let url = self.saved().url.clone();
        let api = Api::new(&url).map_err(super::sync_error)?.with_token(&secrets.token);
        let opened = tokio::time::timeout(Duration::from_secs(30), api.events()).await;
        opened.map_err(|_| failed("the server event stream did not answer"))?.map_err(super::sync_error)
    }

    pub fn account_api(&self, prompt: bool) -> Result<(Api, Secrets)> {
        let secrets = self.secrets(prompt)?;
        let url = self.saved().url.clone();
        let api = Api::new(&url).map_err(super::sync_error)?.with_token(&secrets.token);
        Ok((api, secrets))
    }
}
