use crate::bus::client;
use crate::daemon::Daemon;
use crate::error::{Result, failed};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

pub struct Repos(pub Weak<Daemon>);

impl Repos {
    fn d(&self) -> Result<Arc<Daemon>> {
        self.0.upgrade().ok_or_else(|| failed("the daemon is stopping"))
    }
}

#[zbus::interface(name = "io.github.wallpaper_rs.Repos1")]
impl Repos {
    #[zbus(out_args("json"))]
    async fn list(&self) -> Result<String> {
        self.d()?.run(|d| d.list_repos()).await
    }

    #[zbus(out_args("job"))]
    async fn add(&self, url: String) -> Result<u64> {
        self.d()?.run(move |d| d.add_repo(&url)).await
    }

    #[zbus(out_args("job"))]
    async fn refresh(&self, repo: String) -> Result<u64> {
        self.d()?.run(move |d| d.refresh_repos(&repo)).await
    }

    async fn remove(&self, repo: String, uninstall: bool) -> Result<()> {
        self.d()?.run(move |d| d.remove_repo(&repo, uninstall)).await
    }

    #[zbus(out_args("json"))]
    async fn browse(&self, query: String, offset: u32, limit: u32) -> Result<String> {
        self.d()?.run(move |d| d.browse(&query, offset, limit)).await
    }

    async fn reveal(&self, spec: String) -> Result<()> {
        self.d()?.run(move |d| d.reveal(&spec)).await
    }

    #[zbus(out_args("paths"))]
    async fn request_previews(
        &self,
        specs: Vec<String>,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<HashMap<String, String>> {
        let sender = client(&header);
        self.d()?.run(move |d| d.request_previews(specs, &sender)).await
    }

    #[zbus(out_args("paths"))]
    async fn request_animated_previews(
        &self,
        specs: Vec<String>,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<HashMap<String, String>> {
        let sender = client(&header);
        self.d()?.run(move |d| d.request_animated(specs, &sender)).await
    }

    #[zbus(out_args("job"))]
    async fn install(&self, spec: String, allow_stale: bool, allow_downgrade: bool) -> Result<u64> {
        self.d()?.run(move |d| d.install(&spec, allow_stale, allow_downgrade)).await
    }

    async fn uninstall(&self, id: String, version: String) -> Result<()> {
        self.d()?.run(move |d| d.uninstall(&id, &version)).await
    }

    #[zbus(out_args("json"))]
    async fn list_installed(&self) -> Result<String> {
        self.d()?.run(|d| d.list_installed()).await
    }

    #[zbus(signal)]
    pub async fn preview_ready(emitter: &SignalEmitter<'_>, spec: &str, path: &str) -> zbus::Result<()>;
}
