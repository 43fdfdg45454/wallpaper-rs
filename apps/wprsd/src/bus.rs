use crate::daemon::Daemon;
use crate::streams::HELD;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use zbus::fdo::{self, DBusProxy};
use zbus::message::Header;

static RENDER_PANICKED: AtomicBool = AtomicBool::new(false);

pub(crate) fn client(header: &Header<'_>) -> String {
    header.sender().map_or(HELD, |s| s.as_str()).to_owned()
}

async fn gone(bus: Option<&zbus::Connection>, client: &str) -> bool {
    let Some(bus) = bus else { return false };
    let owner = async { DBusProxy::new(bus).await?.name_has_owner(client.try_into()?).await };
    owner.await.is_ok_and(|owned| !owned)
}

pub(crate) fn track_render_panics() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        RENDER_PANICKED.fetch_or(std::thread::current().name() == Some("wprs-render"), Ordering::Relaxed);
        report(info)
    }));
}

impl Daemon {
    pub(crate) fn rendering(&self) -> anyhow::Result<()> {
        let stopped = RENDER_PANICKED.load(Ordering::Relaxed);
        anyhow::ensure!(!stopped, "the renderer stopped after an error; restart wprsd");
        Ok(())
    }

    pub(crate) async fn run<T: Send + 'static>(
        self: &Arc<Self>,
        work: impl FnOnce(&Daemon) -> anyhow::Result<T> + Send + 'static,
    ) -> fdo::Result<T> {
        let daemon = self.clone();
        let done = tokio::task::spawn_blocking(move || daemon.rendering().and_then(|()| work(&daemon))).await;
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
                let (width, height) = info.size;
                let value = json!({ "width": width, "height": height, "image": r.image, "fit": r.fit.as_str(),
                    "kind": s.kind, "playing": s.playing, "active": st.active(info), "covered": info.covered(),
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
