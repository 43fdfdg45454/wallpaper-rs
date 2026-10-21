use crate::daemon::Daemon;
use std::path::PathBuf;
use std::sync::Arc;
use zbus::fdo;
use zbus::object_server::SignalEmitter;

pub struct Service {
    daemon: Arc<Daemon>,
}

impl Service {
    pub fn new(daemon: Arc<Daemon>) -> Self {
        Self { daemon }
    }

    async fn blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Daemon) -> anyhow::Result<T> + Send + 'static,
    ) -> fdo::Result<T> {
        let daemon = self.daemon.clone();
        tokio::task::spawn_blocking(move || work(&daemon))
            .await
            .map_err(|e| fdo::Error::Failed(format!("internal error: {e}")))?
            .map_err(|e| fdo::Error::Failed(format!("{e:#}")))
    }
}

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl Service {
    async fn request_stream(&self, output: String, width: u32, height: u32) -> fdo::Result<(u32, u64)> {
        let output = self.daemon.check_request(&output, width, height).map_err(fdo::Error::InvalidArgs)?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.daemon.ensure_stream(&output, width, height, Box::new(move |r| drop(tx.send(r))));
        let info = rx
            .await
            .map_err(|_| fdo::Error::Failed("PipeWire thread stopped".into()))?
            .map_err(fdo::Error::Failed)?;
        if info.created {
            tracing::info!(output, width, height, node = info.node_id, serial = info.serial, "stream ready");
        }
        self.blocking(move |d| {
            d.configure_output(&output);
            Ok(())
        })
        .await?;
        Ok((info.node_id, info.serial))
    }

    async fn release_stream(&self, output: String) -> fdo::Result<()> {
        self.blocking(move |d| d.release_stream(output.trim())).await
    }

    async fn set_wallpaper(&self, path: String, output: String, fit: String) -> fdo::Result<()> {
        let fit = match fit.trim() {
            "" => None,
            name => Some(
                name.parse()
                    .map_err(|e: wprs_core::fit::UnknownFit| fdo::Error::InvalidArgs(e.to_string()))?,
            ),
        };
        let path = PathBuf::from(path.strip_prefix("file://").unwrap_or(&path));
        if path.as_os_str().is_empty() {
            return Err(fdo::Error::InvalidArgs("empty path".into()));
        }
        self.blocking(move |d| d.set_wallpaper(path, output.trim(), fit)).await
    }

    async fn set_option(&self, key: String, value: String) -> fdo::Result<()> {
        self.blocking(move |d| d.set_option(&key, &value)).await
    }

    async fn reload(&self) -> fdo::Result<()> {
        self.blocking(|d| d.reload()).await
    }

    async fn get_state(&self) -> String {
        self.daemon.state_json()
    }

    async fn set_paused(
        &self,
        paused: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.daemon.set_paused(paused);
        Ok(self.paused_changed(&emitter).await?)
    }

    async fn set_output_covered(&self, output: String, covered: bool) -> fdo::Result<()> {
        self.daemon.set_covered(&output, covered).map_err(fdo::Error::InvalidArgs)
    }

    async fn set_volume(
        &self,
        volume: f64,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.blocking(move |d| d.set_audio(Some(volume), None)).await?;
        Ok(self.volume_changed(&emitter).await?)
    }

    async fn set_muted(
        &self,
        muted: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.blocking(move |d| d.set_audio(None, Some(muted))).await?;
        Ok(self.muted_changed(&emitter).await?)
    }

    #[zbus(property)]
    async fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    #[zbus(property)]
    async fn adapter(&self) -> String {
        self.daemon.adapter.clone()
    }

    #[zbus(property)]
    async fn paused(&self) -> bool {
        self.daemon.paused()
    }

    #[zbus(property)]
    async fn volume(&self) -> f64 {
        self.daemon.volume()
    }

    #[zbus(property)]
    async fn muted(&self) -> bool {
        self.daemon.muted()
    }
}

#[test]
fn interface_name_matches_core_constant() {
    let literal = format!("#[zbus::interface(name = \"{}\")]", wprs_core::dbus::INTERFACE);
    assert!(include_str!("service.rs").contains(&literal));
}
