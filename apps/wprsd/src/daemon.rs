use crate::compositor::{Compositor, Settings, lock};
use crate::streams::OutputInfo;
use anyhow::Context;
use image::RgbaImage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::SystemTime;
use tokio::sync::{Notify, watch};
use wprs_core::{Config, dbus};
use wprs_pipewire::Exporter;
use wprs_render::Renderer;
use zbus::fdo;

#[derive(Default)]
pub(crate) struct State {
    pub(crate) config: Config,
    pub(crate) config_path: PathBuf,
    pub(crate) config_error: Option<String>,
    pub(crate) outputs: HashMap<String, OutputInfo>,
    pub(crate) images: HashMap<PathBuf, (Option<SystemTime>, Weak<RgbaImage>)>,
    pub(crate) user_paused: bool,
    pub(crate) on_battery: bool,
    pub(crate) locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Props {
    pub(crate) paused: bool,
    pub(crate) volume: f64,
    pub(crate) muted: bool,
}

pub struct Daemon {
    pub(crate) compositor: Compositor,
    pub(crate) state: Mutex<State>,
    pub(crate) exporter: Arc<Exporter>,
    pub(crate) streams_changed: Arc<Notify>,
    pub(crate) props: watch::Sender<Props>,
    pub(crate) adapter: String,
}

impl Daemon {
    pub fn new(config_path: PathBuf) -> anyhow::Result<Self> {
        let (config, config_error) = Config::load(&config_path)
            .map_or_else(|e| (Config::default(), Some(e.to_string())), |config| (config, None));
        match &config_error {
            Some(e) => {
                tracing::error!("cannot use the configuration file; using the defaults until it is fixed");
                tracing::debug!("{e}");
            }
            None => tracing::debug!(path = %config_path.display(), "configuration loaded"),
        }
        let renderer = Renderer::new().context("cannot initialize the GPU renderer")?;
        let adapter = renderer.adapter().to_owned();
        tracing::info!("renderer ready");
        tracing::debug!(adapter, "GPU adapter");
        let exporter = Arc::new(Exporter::start().context("cannot connect to PipeWire")?);
        let compositor = Compositor::start(renderer, exporter.clone(), settings(&config))?;
        let (wake, streams_changed) = (compositor.waker(), Arc::new(Notify::new()));
        let changed = streams_changed.clone();
        exporter.on_change(move || {
            wake();
            changed.notify_one();
        })?;
        let state = State { config, config_path, config_error, ..State::default() };
        let props = watch::Sender::new(state.props());
        Ok(Self { compositor, state: Mutex::new(state), exporter, streams_changed, props, adapter })
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    pub(crate) fn publish(&self) {
        let st = self.state();
        self.props.send_if_modified(|props| std::mem::replace(props, st.props()) != *props);
    }

    pub fn reload(&self) -> anyhow::Result<()> {
        let settings = {
            let mut st = self.state();
            let loaded =
                Config::load(&st.config_path).inspect_err(|e| st.config_error = Some(e.to_string()))?;
            (st.config, st.config_error) = (loaded, None);
            settings(&st.config)
        };
        self.compositor.set_settings(settings);
        self.configure_all();
        self.publish();
        Ok(())
    }
}

impl State {
    fn props(&self) -> Props {
        let playback = &self.config.playback;
        Props { paused: self.user_paused, volume: playback.volume, muted: playback.muted }
    }
}

pub(crate) fn settings(config: &Config) -> Settings {
    let playback = config.playback.sanitized();
    let transition = config.transition.duration();
    Settings { transition, max_fps: playback.max_fps, volume: playback.volume, muted: playback.muted }
}

pub(crate) fn output_name(output: &str) -> fdo::Result<String> {
    let output = output.trim();
    if output.len() > 128 || output.chars().any(char::is_control) {
        return Err(fdo::Error::InvalidArgs("invalid output name".into()));
    }
    Ok(if output.is_empty() { dbus::DEFAULT_OUTPUT.to_owned() } else { output.to_owned() })
}
