use crate::args::{FAILED, Out, fail, fixture, invalid};
use crate::backend::qobject::Backend;
use crate::bus::{self, Daemon};
use cxx_qt_lib::QString;
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;
use wprs_client::Error;
use zeroize::Zeroizing;

impl Backend {
    pub fn add_provider(
        self: Pin<&mut Self>,
        tag: &QString,
        id: &QString,
        kind: &QString,
        options: &QString,
        secret: &QString,
    ) {
        let (id, kind, options) = (id.to_string(), kind.to_string(), options.to_string());
        let secret = Zeroizing::new(secret.to_string());
        self.run(tag, async move {
            let options: HashMap<String, String> =
                serde_json::from_str(&options).map_err(|e| invalid(e.to_string()))?;
            let options = options.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            match bus::daemon().await? {
                Daemon::Bus(c) => keyring(c.providers.add(&id, &kind, options, &secret)).await,
                Daemon::Fixtures => Ok(fixture("Providers1", "Add")),
            }
        });
    }

    pub fn set_secret(self: Pin<&mut Self>, tag: &QString, id: &QString, secret: &QString) {
        let (id, secret) = (id.to_string(), Zeroizing::new(secret.to_string()));
        self.run(tag, async move {
            match bus::daemon().await? {
                Daemon::Bus(c) => keyring(c.providers.set_secret(&id, &secret)).await,
                Daemon::Fixtures => Ok(fixture("Providers1", "SetSecret")),
            }
        });
    }
}

impl Backend {
    pub fn scene_secret(
        self: Pin<&mut Self>,
        tag: &QString,
        output: &QString,
        key: &QString,
        secret: &QString,
    ) {
        let (output, key) = (output.to_string(), key.to_string());
        let value = Zeroizing::new(serde_json::Value::String(secret.to_string()).to_string());
        self.run(tag, async move {
            match bus::daemon().await? {
                Daemon::Bus(c) => keyring(c.daemon.set_scene_property(&output, &key, &value)).await,
                Daemon::Fixtures => Ok(fixture("Daemon1", "SetSceneProperty")),
            }
        });
    }

    pub fn passphrase(self: Pin<&mut Self>, tag: &QString, member: &QString, passphrase: &QString) {
        let (member, passphrase) = (member.to_string(), Zeroizing::new(passphrase.to_string()));
        self.run(tag, async move {
            match (bus::daemon().await?, member.as_str()) {
                (Daemon::Bus(c), "SetEscrow") => keyring(c.sync.set_escrow(&passphrase)).await,
                (Daemon::Bus(c), "Restore") => keyring(c.sync.restore(&passphrase)).await,
                (Daemon::Fixtures, "SetEscrow" | "Restore") => Ok(fixture("Sync1", &member)),
                _ => Err(invalid(format!("{member} takes no passphrase"))),
            }
        });
    }
}

async fn keyring<T: Out>(call: impl Future<Output = zbus::Result<T>>) -> Result<String, Error> {
    let late = |_| Error { name: FAILED.into(), message: "the keyring did not answer in time".into() };
    let reply = tokio::time::timeout(Duration::from_secs(120), call).await.map_err(late)?;
    reply.map(|r| r.out().to_string()).map_err(fail)
}
