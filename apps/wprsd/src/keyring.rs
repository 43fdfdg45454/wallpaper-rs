use crate::compositor::lock;
use crate::error::{Error, Result};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wprs_core::dbus::error::KEYRING_UNAVAILABLE;
use zbus::proxy::CacheProperties;
use zbus::{ObjectPath, OwnedObjectPath, Variant};
use zeroize::{Zeroize, Zeroizing};

const SERVICE: &str = "org.freedesktop.secrets";
const SCHEMA: &str = "io.github.wallpaper_rs.Provider";
const COLLECTIONS: [&str; 2] =
    ["/org/freedesktop/secrets/aliases/default", "/org/freedesktop/secrets/collection/login"];
const WAIT: Duration = Duration::from_secs(20);
const LOOKUP: Duration = Duration::from_secs(10);
const WRITE: Duration = Duration::from_secs(100);
const SILENCE: Duration = Duration::from_secs(60);
const ABSENT: &str = "no Secret Service is available to store the secret";
const SLOW: &str = "the keyring did not answer in time";

type Secret = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);
type Paths = Vec<OwnedObjectPath>;

mod proxy;
mod write;

use proxy::{CollectionProxy, ItemProxy, PromptProxy, ServiceProxy};

pub enum Stored {
    Secret(Zeroizing<String>),
    Missing,
    Locked,
}

pub struct Keyring {
    bus: zbus::Connection,
    lookup: Duration,
    write: Duration,
    kind: (&'static str, &'static str),
    silent: Arc<Mutex<Option<Instant>>>,
    known: Arc<Mutex<HashMap<String, bool>>>,
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::Named(KEYRING_UNAVAILABLE, message.into())
}

fn refused(e: zbus::Error) -> Error {
    unavailable(format!("the keyring refused the request: {e}"))
}

async fn bounded<T>(limit: Duration, work: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(limit, work).await.unwrap_or_else(|_| Err(unavailable(SLOW)))
}

impl Keyring {
    pub fn new(bus: zbus::Connection) -> Self {
        let (silent, known) = (Arc::default(), Arc::default());
        Keyring { bus, lookup: LOOKUP, write: WRITE, kind: (SCHEMA, "provider"), silent, known }
    }

    pub fn server(bus: zbus::Connection) -> Self {
        Keyring { kind: ("io.github.wallpaper_rs.Server", "server"), ..Keyring::new(bus) }
    }

    pub fn properties(&self) -> Self {
        let kind = ("io.github.wallpaper_rs.Property", "property");
        Keyring { kind, silent: self.silent.clone(), ..Keyring::new(self.bus.clone()) }
    }

    pub fn known(&self, id: &str) -> Option<bool> {
        lock(&self.known).get(id).copied()
    }

    fn remember(&self, id: &str, stored: bool) {
        lock(&self.known).insert(id.to_owned(), stored);
    }

    async fn asked<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        if lock(&self.silent).is_some_and(|until| Instant::now() < until) {
            return Err(unavailable(SLOW));
        }
        let answer = tokio::time::timeout(self.lookup, work).await;
        *lock(&self.silent) = answer.is_err().then(|| Instant::now() + SILENCE);
        answer.unwrap_or_else(|_| Err(unavailable(SLOW)))
    }

    fn attributes<'a>(&self, id: &'a str) -> HashMap<&'a str, &'a str> {
        HashMap::from([("xdg:schema", self.kind.0), (self.kind.1, id)])
    }

    async fn service(&self) -> zbus::Result<ServiceProxy<'static>> {
        ServiceProxy::new(&self.bus).await
    }

    async fn search(&self, id: &str) -> zbus::Result<(Paths, Paths)> {
        self.service().await?.search_items(self.attributes(id)).await
    }

    pub async fn has(&self, id: &str) -> Result<bool> {
        let found = self.asked(async { Ok(self.search(id).await) }).await?;
        let stored = found.is_ok_and(|(open, locked)| !open.is_empty() || !locked.is_empty());
        self.remember(id, stored);
        Ok(stored)
    }

    pub async fn read(&self, id: &str, prompt: bool) -> Result<Stored> {
        match prompt {
            true => bounded(self.write, self.fetch(id, prompt)).await,
            false => self.asked(self.fetch(id, prompt)).await,
        }
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
pub(crate) mod tests;
