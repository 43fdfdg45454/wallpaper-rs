//! D-Bus interface of the daemon. See `docs/specs/dbus.md`.

use crate::daemon::Daemon;
use std::path::PathBuf;
use std::sync::Arc;
use zbus::fdo;

pub struct Service {
    daemon: Arc<Daemon>,
}

impl Service {
    pub fn new(daemon: Arc<Daemon>) -> Self {
        Self { daemon }
    }

    /// Runs blocking daemon work off the async executor.
    async fn blocking<T, F>(&self, f: F) -> fdo::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Daemon) -> anyhow::Result<T> + Send + 'static,
    {
        let daemon = self.daemon.clone();
        tokio::task::spawn_blocking(move || f(&daemon))
            .await
            .map_err(|e| fdo::Error::Failed(format!("internal error: {e}")))?
            .map_err(|e| fdo::Error::Failed(format!("{e:#}")))
    }
}

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl Service {
    /// Creates (or reuses) a video stream for `output` at `width` x `height`
    /// physical pixels and returns its PipeWire node id and object serial.
    async fn request_stream(&self, output: String, width: u32, height: u32) -> fdo::Result<(u32, u64)> {
        let output = self
            .daemon
            .check_request(&output, width, height)
            .map_err(fdo::Error::InvalidArgs)?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.daemon.ensure_stream(
            &output,
            width,
            height,
            Box::new(move |r| {
                let _ = tx.send(r);
            }),
        );
        let info = rx
            .await
            .map_err(|_| fdo::Error::Failed("PipeWire thread stopped".into()))?
            .map_err(fdo::Error::Failed)?;
        if info.created {
            tracing::info!(
                output,
                width,
                height,
                node = info.node_id,
                serial = info.serial,
                "stream ready"
            );
            let render_output = output.clone();
            self.blocking(move |d| d.render_output(&render_output)).await?;
        }
        Ok((info.node_id, info.serial))
    }

    /// Sets the image for `output` (empty for every output). `fit` is one of
    /// fill, fit, stretch, center, tile, or empty to keep the current one.
    async fn set_wallpaper(&self, path: String, output: String, fit: String) -> fdo::Result<()> {
        let fit = if fit.trim().is_empty() {
            None
        } else {
            Some(
                fit.parse()
                    .map_err(|e: wprs_core::fit::UnknownFit| fdo::Error::InvalidArgs(e.to_string()))?,
            )
        };
        if path.trim().is_empty() {
            return Err(fdo::Error::InvalidArgs("empty path".into()));
        }
        let path = PathBuf::from(path.strip_prefix("file://").unwrap_or(&path));
        let output = output.trim().to_owned();
        self.blocking(move |d| d.set_wallpaper(path, &output, fit)).await
    }

    /// Re-reads the configuration file.
    async fn reload(&self) -> fdo::Result<()> {
        self.blocking(|d| d.reload()).await
    }

    /// JSON document describing the daemon state.
    async fn get_state(&self) -> String {
        self.daemon.state_json()
    }

    #[zbus(property)]
    async fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    #[zbus(property)]
    async fn adapter(&self) -> String {
        self.daemon.adapter().to_owned()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn interface_name_matches_core_constant() {
        // The interface macro needs a literal; keep it in sync with wprs-core.
        let src = include_str!("service.rs");
        let literal = format!("#[zbus::interface(name = \"{}\")]", wprs_core::dbus::INTERFACE);
        assert!(src.contains(&literal));
    }
}
