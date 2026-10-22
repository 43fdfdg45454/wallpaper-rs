use crate::error::{Error, Result};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use wprs_core::dbus::error::KEYRING_UNAVAILABLE;
use zbus::proxy::CacheProperties;
use zbus::{ObjectPath, OwnedObjectPath, OwnedVariant, Variant};
use zeroize::{Zeroize, Zeroizing};

const SERVICE: &str = "org.freedesktop.secrets";
const SCHEMA: &str = "io.github.wallpaper_rs.Provider";
const COLLECTIONS: [&str; 2] =
    ["/org/freedesktop/secrets/aliases/default", "/org/freedesktop/secrets/collection/login"];
const WAIT: Duration = Duration::from_secs(20);
const LOOKUP: Duration = Duration::from_secs(10);
const WRITE: Duration = Duration::from_secs(100);
const ABSENT: &str = "no Secret Service is available to store the secret";
const SLOW: &str = "the keyring did not answer in time";

type Secret = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);
type Paths = Vec<OwnedObjectPath>;

#[zbus::proxy(
    interface = "org.freedesktop.Secret.Service",
    default_service = "org.freedesktop.secrets",
    default_path = "/org/freedesktop/secrets"
)]
trait Service {
    fn open_session(
        &self,
        algorithm: &str,
        input: &Variant<'_>,
    ) -> zbus::Result<(OwnedVariant, OwnedObjectPath)>;
    fn search_items(&self, attributes: HashMap<&str, &str>) -> zbus::Result<(Paths, Paths)>;
    fn unlock(&self, objects: &[OwnedObjectPath]) -> zbus::Result<(Paths, OwnedObjectPath)>;
    fn get_secrets(
        &self,
        items: &[OwnedObjectPath],
        session: &ObjectPath<'_>,
    ) -> zbus::Result<HashMap<OwnedObjectPath, Secret>>;
}

#[zbus::proxy(interface = "org.freedesktop.Secret.Collection", default_service = "org.freedesktop.secrets")]
trait Collection {
    fn create_item(
        &self,
        properties: HashMap<&str, Variant<'_>>,
        secret: &Secret,
        replace: bool,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
    #[zbus(property)]
    fn locked(&self) -> zbus::Result<bool>;
}

#[zbus::proxy(interface = "org.freedesktop.Secret.Item", default_service = "org.freedesktop.secrets")]
trait Item {
    fn delete(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(interface = "org.freedesktop.Secret.Prompt", default_service = "org.freedesktop.secrets")]
trait Prompt {
    fn prompt(&self, window_id: &str) -> zbus::Result<()>;
    fn dismiss(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn completed(&self, dismissed: bool, result: Variant<'_>) -> zbus::Result<()>;
}

pub enum Stored {
    Secret(Zeroizing<String>),
    Missing,
    Locked,
}

pub struct Keyring {
    bus: zbus::Connection,
    lookup: Duration,
    write: Duration,
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::Named(KEYRING_UNAVAILABLE, message.into())
}

fn refused(e: zbus::Error) -> Error {
    unavailable(format!("the keyring refused the request: {e}"))
}

fn attributes(id: &str) -> HashMap<&str, &str> {
    HashMap::from([("xdg:schema", SCHEMA), ("provider", id)])
}

async fn bounded<T>(limit: Duration, work: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(limit, work).await.unwrap_or_else(|_| Err(unavailable(SLOW)))
}

impl Keyring {
    pub fn new(bus: zbus::Connection) -> Self {
        Keyring { bus, lookup: LOOKUP, write: WRITE }
    }

    async fn service(&self) -> zbus::Result<ServiceProxy<'static>> {
        ServiceProxy::new(&self.bus).await
    }

    async fn search(&self, id: &str) -> zbus::Result<(Paths, Paths)> {
        self.service().await?.search_items(attributes(id)).await
    }

    pub async fn has(&self, id: &str) -> Result<bool> {
        let found =
            tokio::time::timeout(self.lookup, self.search(id)).await.map_err(|_| unavailable(SLOW))?;
        Ok(found.is_ok_and(|(open, locked)| !open.is_empty() || !locked.is_empty()))
    }

    pub async fn read(&self, id: &str, prompt: bool) -> Result<Stored> {
        bounded(if prompt { self.write } else { self.lookup }, self.fetch(id, prompt)).await
    }

    async fn fetch(&self, id: &str, prompt: bool) -> Result<Stored> {
        let Ok((mut open, locked)) = self.search(id).await else { return Ok(Stored::Missing) };
        if open.is_empty() && !locked.is_empty() {
            if !prompt {
                return Ok(Stored::Locked);
            }
            self.unlock(&locked).await?;
            open = locked;
        }
        let Some(item) = open.into_iter().next() else { return Ok(Stored::Missing) };
        let service = self.service().await.map_err(refused)?;
        let (_, session) = service.open_session("plain", &Variant::from("")).await.map_err(refused)?;
        let found = service.get_secrets(std::slice::from_ref(&item), &session).await;
        self.close(&session).await;
        let (.., value, _) = found.map_err(refused)?.remove(&item).ok_or_else(|| unavailable("no secret"))?;
        String::from_utf8(value).map(|text| Stored::Secret(Zeroizing::new(text))).map_err(|e| {
            e.into_bytes().zeroize();
            unavailable("the stored secret is not text")
        })
    }

    pub async fn store(&self, id: &str, secret: &str) -> Result<()> {
        bounded(self.write, self.create(id, secret)).await
    }

    async fn create(&self, id: &str, secret: &str) -> Result<()> {
        let service = self.service().await.map_err(|_| unavailable(ABSENT))?;
        let collection = self.collection().await?;
        if collection.locked().await.map_err(refused)? {
            self.unlock(&[collection.inner().path().to_owned().into()]).await?;
        }
        let (_, session) = service.open_session("plain", &Variant::from("")).await.map_err(refused)?;
        let properties = HashMap::from([
            ("org.freedesktop.Secret.Item.Label", Variant::from(format!("wallpaper-rs: {id}"))),
            ("org.freedesktop.Secret.Item.Attributes", Variant::from(attributes(id))),
        ]);
        let mut value = (session.clone(), Vec::new(), secret.as_bytes().to_vec(), "text/plain".to_owned());
        let created = collection.create_item(properties, &value, true).await;
        value.2.zeroize();
        self.close(&session).await;
        self.complete(created.map_err(refused)?.1).await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        bounded(self.write, async {
            let Ok((open, locked)) = self.search(id).await else { return Ok(()) };
            if !locked.is_empty() {
                self.unlock(&locked).await?;
            }
            for path in open.into_iter().chain(locked) {
                self.complete(self.item(path).await?.delete().await.map_err(refused)?).await?;
            }
            Ok(())
        })
        .await
    }

    pub async fn discard(&self, id: &str) -> Result<bool> {
        bounded(self.write, async {
            let Ok((open, locked)) = self.search(id).await else { return Ok(true) };
            for path in open {
                let prompt = self.item(path).await?.delete().await.map_err(refused)?;
                if prompt.as_str() != "/" {
                    let pending = PromptProxy::builder(&self.bus).path(prompt).build().await;
                    pending.map_err(refused)?.dismiss().await.ok();
                    return Ok(false);
                }
            }
            Ok(locked.is_empty())
        })
        .await
    }

    async fn item(&self, path: OwnedObjectPath) -> Result<ItemProxy<'static>> {
        let item = ItemProxy::builder(&self.bus).path(path);
        item.cache_properties(CacheProperties::No).build().await.map_err(refused)
    }

    async fn collection(&self) -> Result<CollectionProxy<'static>> {
        for path in COLLECTIONS {
            let builder =
                CollectionProxy::builder(&self.bus).path(path).cache_properties(CacheProperties::No);
            let proxy = builder.build().await.map_err(refused)?;
            if proxy.locked().await.is_ok() {
                return Ok(proxy);
            }
        }
        Err(unavailable(ABSENT))
    }

    async fn unlock(&self, objects: &[OwnedObjectPath]) -> Result<()> {
        let service = self.service().await.map_err(refused)?;
        self.complete(service.unlock(objects).await.map_err(refused)?.1).await
    }

    async fn complete(&self, prompt: OwnedObjectPath) -> Result<()> {
        if prompt.as_str() == "/" {
            return Ok(());
        }
        let proxy = PromptProxy::builder(&self.bus).path(prompt).build().await.map_err(refused)?;
        let mut completed = proxy.receive_completed().await.map_err(refused)?;
        proxy.prompt("").await.map_err(refused)?;
        match tokio::time::timeout(WAIT, completed.next()).await {
            Ok(Some(signal)) if signal.args().is_ok_and(|a| !a.dismissed) => Ok(()),
            Ok(_) => Err(unavailable("unlock the keyring and try again")),
            Err(_) => {
                proxy.dismiss().await.ok();
                Err(unavailable("unlock the keyring and try again"))
            }
        }
    }

    async fn close(&self, session: &ObjectPath<'_>) {
        let interface = Some("org.freedesktop.Secret.Session");
        self.bus.call_method(Some(SERVICE), session, interface, "Close", &()).await.ok();
    }
}

#[cfg(test)]
mod tests;
