use crate::daemon::Daemon;
use crate::error::{Error, failed};
use crate::service::Service;
use crate::streams::HELD;
use std::sync::Arc;
use wprs_core::dbus;
use wprs_core::model::{MODEL, OutputState, Shown, State};
use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

pub(crate) fn rendering(running: bool) -> Result<(), Error> {
    if running { Ok(()) } else { Err(failed("the renderer stopped after an error; restart wprsd")) }
}

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
    pub(crate) async fn run<T: Send + 'static, E: Into<Error>>(
        self: &Arc<Self>,
        work: impl FnOnce(&Arc<Daemon>) -> Result<T, E> + Send + 'static,
    ) -> Result<T, Error> {
        let daemon = self.clone();
        let done = tokio::task::spawn_blocking(move || work(&daemon).map_err(Into::into)).await;
        done.map_err(|e| failed(format!("internal error: {e}")))?
    }

    pub(crate) async fn render<T: Send + 'static, E: Into<Error>>(
        self: &Arc<Self>,
        work: impl FnOnce(&Arc<Daemon>) -> Result<T, E> + Send + 'static,
    ) -> Result<T, Error> {
        rendering(self.compositor.running())?;
        self.run(work).await
    }

    pub(crate) async fn request(
        self: &Arc<Self>,
        output: String,
        size: (u32, u32),
        client: String,
        bus: Option<&zbus::Connection>,
    ) -> Result<(u32, u64), Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let (claimant, reply) = (client.clone(), Box::new(|r| drop(tx.send(r))));
        let output = self.render(move |d| d.claim(&output, &claimant, size, reply)).await?;
        let ready = match gone(bus, &client).await {
            true => Err("the client left the bus".into()),
            false => rx.await.unwrap_or_else(|_| Err("PipeWire thread stopped".into())),
        };
        let info = match ready {
            Ok(info) => info,
            Err(e) => {
                self.unclaim(&mut self.state(), &output, &client, 1).ok();
                return Err(failed(e));
            }
        };
        if info.created {
            tracing::info!(output, node = info.node_id, serial = info.serial, "stream ready");
        }
        self.run(move |d| {
            d.configure_output(&output);
            Ok::<_, Error>(())
        })
        .await?;
        Ok((info.node_id, info.serial))
    }

    pub fn state_json(&self) -> String {
        let status = self.compositor.status();
        let serial = self.changes.serial();
        let jobs = self.jobs.active();
        let st = self.state();
        let output = |(name, info): (&String, &crate::streams::OutputInfo)| {
            let (r, s) = (st.config.resolve(name), status.get(name).cloned().unwrap_or_default());
            let ((width, height), [red, green, blue]) = (info.size, r.background);
            let kind = match s.kind {
                "image" => Shown::Image,
                "video" => Shown::Video,
                "scene" => Shown::Scene,
                _ => Shown::Color,
            };
            let section = st.config.outputs.get(name).filter(|o| o.image.is_some() || o.playlist.is_some());
            let preset = section.unwrap_or(&st.config.default).preset.clone();
            let item = r.image.as_ref().and_then(|image| st.items.get(image).cloned());
            let credit = r.image.as_deref().and_then(|image| self.credit(image));
            let mut state = OutputState {
                width,
                height,
                image: r.image,
                fit: r.fit,
                background: format!("#{red:02x}{green:02x}{blue:02x}"),
                kind,
                playing: s.playing,
                active: st.active(info),
                covered: info.covered(),
                clients: info.claims.len() as u32,
                error: s.error,
                item,
                preset,
                playlist: None,
                credit,
                edited: info.edited(),
                screen: info.screen.clone(),
            };
            crate::rotation::overlay(&st, name, &mut state);
            (name.clone(), state)
        };
        let playback = &st.config.playback;
        let state = State {
            model: MODEL,
            version: env!("CARGO_PKG_VERSION").into(),
            adapter: self.adapter.clone(),
            config: st.config_path.clone(),
            config_error: st.config_error.clone(),
            library_error: self.store.error.clone(),
            serial,
            paused: st.user_paused,
            on_battery: st.on_battery,
            locked: st.locked,
            volume: playback.volume,
            muted: playback.muted,
            profile: st.config.profile.clone(),
            jobs,
            options: serde_json::to_value(st.config.options())
                .and_then(serde_json::from_value)
                .unwrap_or_default(),
            outputs: st.outputs.iter().map(output).collect(),
        };
        serde_json::to_string(&state).unwrap_or_default()
    }
}
