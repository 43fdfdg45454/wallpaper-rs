use crate::daemon::Daemon;
use crate::error::{Result, failed};
use std::sync::{Arc, Weak};

pub struct Sync(pub Weak<Daemon>);

impl Sync {
    fn d(&self) -> Result<Arc<Daemon>> {
        self.0.upgrade().ok_or_else(|| failed("the daemon is stopping"))
    }
}

#[zbus::interface(name = "io.github.wallpaper_rs.Sync1")]
impl Sync {
    #[zbus(out_args("job"))]
    async fn add_server(&self, url: String) -> Result<u64> {
        self.d()?.run(move |d| d.add_server(&url)).await
    }

    #[zbus(out_args("user_code", "verification_uri", "job"))]
    async fn link(&self, name: String, plaintext: bool) -> Result<(String, String, u64)> {
        self.d()?.run(move |d| d.link_device(&name, plaintext)).await
    }

    #[zbus(out_args("job"))]
    async fn approve(&self, user_code: String) -> Result<u64> {
        self.d()?.run(move |d| d.approve(&user_code)).await
    }

    #[zbus(out_args("json"))]
    async fn get_status(&self) -> Result<String> {
        self.d()?.run(|d| d.account_status()).await
    }

    #[zbus(out_args("job"))]
    async fn sync_now(&self) -> Result<u64> {
        self.d()?.run(|d| d.sync_now()).await
    }

    async fn set_paused(&self, paused: bool) -> Result<()> {
        self.d()?.run(move |d| d.set_sync_paused(paused)).await
    }

    #[zbus(out_args("code", "job"))]
    async fn join(&self, device: String) -> Result<(String, u64)> {
        self.d()?.run(move |d| d.join(&device)).await
    }

    #[zbus(out_args("job"))]
    async fn share_key(&self, device: String, code: String) -> Result<u64> {
        self.d()?.run(move |d| d.share_key(&device, &code)).await
    }

    #[zbus(out_args("job"))]
    async fn upload(&self, path: String) -> Result<u64> {
        self.d()?.run(move |d| d.upload(&path)).await
    }

    async fn remove_server(&self, wipe: bool) -> Result<()> {
        self.d()?.run(move |d| d.remove_server(wipe)).await
    }
}
