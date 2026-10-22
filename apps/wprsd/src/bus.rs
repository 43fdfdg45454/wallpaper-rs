use crate::daemon::Daemon;
use crate::service::Service;
use crate::streams::HELD;
use serde_json::json;
use std::sync::Arc;
use wprs_core::dbus;
use zbus::fdo::{self, DBusProxy};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

pub(crate) fn client(header: &Header<'_>) -> String {
    header.sender().map_or(HELD, |s| s.as_str()).to_owned()
}

async fn gone(bus: Option<&zbus::Connection>, client: &str) -> bool {
    let Some(bus) = bus else { return false };
    let owner = async { DBusProxy::new(bus).await?.name_has_owner(client.try_into()?).await };
    owner.await.is_ok_and(|owned| !owned)
}

pub(crate) async fn announce_streams(connection: zbus::Connection, daemon: Arc<Daemon>) {
    let Ok(emitter) = SignalEmitter::new(&connection, dbus::OBJECT_PATH) else { return };
    let (mut seen, mut waiting) = (daemon.exporter.generation(), Vec::new());
    loop {
        daemon.streams_changed.notified().await;
        let generation = daemon.exporter.generation();
        if generation != seen {
            (seen, waiting) = (generation, daemon.state().outputs.keys().cloned().collect());
        }
        for output in std::mem::take(&mut waiting) {
            match daemon.exporter.stream_info(&output) {
                Some(info) => {
                    tracing::info!(output, node = info.node_id, serial = info.serial, "stream recreated");
                    let sent = Service::stream_changed(&emitter, &output, info.node_id, info.serial).await;
                    sent.unwrap_or_else(|e| tracing::warn!("cannot announce a recreated stream: {e}"));
                }
                None if daemon.state().outputs.contains_key(&output) => waiting.push(output),
                None => {}
            }
        }
    }
}

impl Daemon {
    pub(crate) async fn run<T: Send + 'static>(
        self: &Arc<Self>,
        work: impl FnOnce(&Daemon) -> anyhow::Result<T> + Send + 'static,
    ) -> fdo::Result<T> {
        let daemon = self.clone();
        let done = tokio::task::spawn_blocking(move || work(&daemon)).await;
        done.map_err(|e| fdo::Error::Failed(format!("internal error: {e}")))?
            .map_err(|e| e.downcast().unwrap_or_else(|e| fdo::Error::Failed(format!("{e:#}"))))
    }

    pub(crate) async fn request(
        self: &Arc<Self>,
        output: String,
        size: (u32, u32),
        client: String,
        bus: Option<&zbus::Connection>,
    ) -> fdo::Result<(u32, u64)> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let (claimant, reply) = (client.clone(), Box::new(|r| drop(tx.send(r))));
        let output = self.run(move |d| Ok(d.claim(&output, &claimant, size, reply)?)).await?;
        let ready = match gone(bus, &client).await {
            true => Err("the client left the bus".into()),
            false => rx.await.unwrap_or_else(|_| Err("PipeWire thread stopped".into())),
        };
        let info = match ready {
            Ok(info) => info,
            Err(e) => {
                self.unclaim(&mut self.state(), &output, &client, 1).ok();
                return Err(fdo::Error::Failed(e));
            }
        };
        if info.created {
            tracing::info!(output, node = info.node_id, serial = info.serial, "stream ready");
        }
        self.run(move |d| {
            d.configure_output(&output);
            Ok(())
        })
        .await?;
        Ok((info.node_id, info.serial))
    }

    pub fn state_json(&self) -> String {
        let status = self.compositor.status();
        let st = self.state();
        let outputs: serde_json::Map<_, _> = (st.outputs.iter())
            .map(|(name, info)| {
                let (r, s) = (st.config.resolve(name), status.get(name).cloned().unwrap_or_default());
                let ((width, height), [red, green, blue]) = (info.size, r.background);
                let value = json!({ "width": width, "height": height, "image": r.image, "fit": r.fit.as_str(),
                    "background": format!("#{red:02x}{green:02x}{blue:02x}"), "kind": s.kind,
                    "playing": s.playing, "active": st.active(info), "covered": info.covered(),
                    "clients": info.claims.len(), "error": s.error });
                (name.clone(), value)
            })
            .collect();
        let playback = &st.config.playback;
        json!({ "version": env!("CARGO_PKG_VERSION"), "adapter": self.adapter, "config": st.config_path,
            "config_error": st.config_error, "paused": st.user_paused, "on_battery": st.on_battery,
            "locked": st.locked, "volume": playback.volume, "muted": playback.muted,
            "options": st.config.options(), "outputs": outputs })
        .to_string()
    }
}
