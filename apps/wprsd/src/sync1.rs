use crate::daemon::Daemon;
use crate::error::{Result, failed};
use std::sync::{Arc, Weak};
use zeroize::Zeroizing;

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
    async fn preview_code(&self, user_code: String) -> Result<u64> {
        self.d()?.run(move |d| d.preview_code(&user_code)).await
    }

    #[zbus(out_args("job"))]
    async fn approve(&self, user_code: String) -> Result<u64> {
        self.d()?.run(move |d| d.approve(&user_code)).await
    }

    #[zbus(out_args("job"))]
    async fn approve_device(&self, user_code: String, password: String) -> Result<u64> {
        let password = Zeroizing::new(password);
        self.d()?.run(move |d| d.approve_with(&user_code, password)).await
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

    #[zbus(out_args("job"))]
    async fn join(&self, device: String) -> Result<u64> {
        self.d()?.run(move |d| d.join(&device)).await
    }

    #[zbus(out_args("job"))]
    async fn share_key(&self, device: String) -> Result<u64> {
        self.d()?.run(move |d| d.share_key(&device)).await
    }

    #[zbus(out_args("job"))]
    async fn confirm_share(&self, device: String, code: String) -> Result<u64> {
        self.d()?.run(move |d| d.confirm_share(&device, &code)).await
    }

    #[zbus(out_args("job"))]
    async fn rotate(&self) -> Result<u64> {
        self.d()?.run(|d| d.rotate_now()).await
    }

    #[zbus(out_args("job"))]
    async fn reset(&self) -> Result<u64> {
        self.d()?.run(|d| d.reset_sync()).await
    }

    #[zbus(out_args("job"))]
    async fn confirm_repo(&self, repo: String, add: bool) -> Result<u64> {
        self.d()?.run(move |d| d.confirm_repo(&repo, add)).await
    }

    async fn share_secret(&self, provider: String, share: bool) -> Result<()> {
        self.d()?.run(move |d| d.share_secret(&provider, share)).await
    }

    #[zbus(out_args("job"))]
    async fn set_escrow(&self, passphrase: String) -> Result<u64> {
        let passphrase = Zeroizing::new(passphrase);
        self.d()?.run(move |d| d.set_escrow(passphrase)).await
    }

    #[zbus(out_args("job"))]
    async fn restore(&self, passphrase: String) -> Result<u64> {
        let passphrase = Zeroizing::new(passphrase);
        self.d()?.run(move |d| d.restore(passphrase)).await
    }

    #[zbus(out_args("job"))]
    async fn share_files(&self, targets: Vec<String>, share: bool) -> Result<u64> {
        self.d()?.run(move |d| d.share_files(&targets, share)).await
    }

    #[zbus(out_args("json"))]
    async fn list_files(&self) -> Result<String> {
        self.d()?.run(|d| d.list_files()).await
    }

    #[zbus(out_args("job"))]
    async fn fetch_file(&self, file: String) -> Result<u64> {
        self.d()?.run(move |d| d.fetch_file(&file)).await
    }

    #[zbus(out_args("job"))]
    async fn upload(&self, path: String) -> Result<u64> {
        self.d()?.run(move |d| d.upload(&path)).await
    }

    async fn remove_server(&self, wipe: bool) -> Result<()> {
        self.d()?.run(move |d| d.remove_server(wipe)).await
    }
}
