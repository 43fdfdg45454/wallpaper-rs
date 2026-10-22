use crate::daemon::Daemon;
use crate::error::{Result, failed};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use zeroize::Zeroizing;

pub struct Providers(pub Weak<Daemon>);

impl Providers {
    fn d(&self) -> Result<Arc<Daemon>> {
        self.0.upgrade().ok_or_else(|| failed("the daemon is stopping"))
    }
}

#[zbus::interface(name = "io.github.wallpaper_rs.Providers1")]
impl Providers {
    #[zbus(out_args("json"))]
    async fn list(&self) -> Result<String> {
        self.d()?.list_providers().await
    }

    #[zbus(out_args("source"))]
    async fn add(
        &self,
        id: String,
        kind: String,
        options: HashMap<String, String>,
        secret: String,
    ) -> Result<String> {
        let secret = Zeroizing::new(secret);
        self.d()?.add_provider(id, kind, options, secret).await
    }

    async fn set(&self, id: String, key: String, value: String) -> Result<()> {
        self.d()?.set_provider(id, key, value).await
    }

    async fn remove(&self, id: String, purge: bool) -> Result<()> {
        self.d()?.remove_provider(id, purge).await
    }

    #[zbus(out_args("job"))]
    async fn sync(&self, id: String) -> Result<u64> {
        self.d()?.sync_providers(id).await
    }

    async fn set_secret(&self, id: String, secret: String) -> Result<()> {
        let secret = Zeroizing::new(secret);
        self.d()?.set_secret(id, secret).await
    }

    async fn forget_secret(&self, id: String) -> Result<()> {
        self.d()?.forget_secret(id).await
    }
}
