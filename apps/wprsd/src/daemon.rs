use crate::changes::Changes;
use crate::compositor::{Compositor, Settings, lock};
use crate::folders::Folders;
use crate::jobs::Jobs;
use crate::rotation::{Playing, Rotations};
use crate::store::Store;
use crate::streams::OutputInfo;
use crate::thumbs::Thumbs;
use anyhow::Context;
use image::RgbaImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::SystemTime;
use tokio::sync::{Notify, watch};
use wprs_core::{Config, Models, dbus};
use wprs_format::Installed;
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
    pub(crate) pixels: HashMap<PathBuf, (Option<SystemTime>, Weak<wprs_render::Pixels>)>,
    pub(crate) user_paused: bool,
    pub(crate) on_battery: bool,
    pub(crate) locked: bool,
    pub(crate) items: HashMap<PathBuf, String>,
    pub(crate) installed: Vec<Installed>,
    pub(crate) playing: HashMap<String, Playing>,
    pub(crate) held: HashMap<String, String>,
    pub(crate) sidecars: crate::edits::Sidecars,
    pub(crate) drafts: HashMap<String, crate::drafts::Draft>,
    pub(crate) depths: HashMap<PathBuf, crate::relief::Cached>,
    pub(crate) scenes: HashMap<PathBuf, ((i64, u64), Weak<crate::scenes::ScenePack>)>,
    pub(crate) keys: crate::inputs::Keys,
    pub(crate) modes: HashMap<String, ((u32, u32), u32)>,
    pub(crate) resized: Vec<(String, (u32, u32))>,
    pub(crate) turns: HashMap<String, tokio::time::Instant>,
    pub(crate) missed: std::collections::HashSet<String>,
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
    pub(crate) tier: (&'static str, bool),
    pub(crate) store: Store,
    pub(crate) changes: Changes,
    pub(crate) jobs: Jobs,
    pub(crate) thumbs: Arc<Thumbs>,
    pub(crate) folders: Folders,
    pub(crate) rotations: Rotations,
    pub(crate) remote: crate::remote::Remote,
    pub(crate) account: crate::account::Account,
    pub(crate) turned: Notify,
}

impl Daemon {
    pub fn new(config_path: PathBuf, display: Option<wprs_render::Display>) -> anyhow::Result<Self> {
        let store = Store::open();
        let closed = || Config { models: Models { allow_download: false }, ..Config::default() };
        let (config, config_error) = load(&config_path, &store)
            .map_or_else(|e| (closed(), Some(e.to_string())), |config| (config, None));
        match &config_error {
            Some(e) => {
                tracing::error!("cannot use the configuration file; using the defaults until it is fixed");
                tracing::debug!("{e}");
            }
            None => tracing::debug!(path = %config_path.display(), "configuration loaded"),
        }
        let renderer =
            Renderer::with_display(display).context("cannot initialize the GPU renderer")?.install();
        let adapter = renderer.adapter().to_owned();
        let calibrated = renderer.calibrate(wprs_core::gpu_tier().as_deref());
        let tier = (["t0", "t1", "t2", "t3"][calibrated.tier as usize], calibrated.measured());
        tracing::info!("renderer ready");
        tracing::debug!(adapter, tier = tier.0, unit_ms = calibrated.unit_ms, "GPU adapter");
        let changes = Changes::new();
        let exporter = Arc::new(Exporter::start().context("cannot connect to PipeWire")?);
        crate::streams::gpu_buffers(renderer).into_iter().for_each(|gpu| exporter.use_dmabuf(gpu));
        crate::streams::timelines(&exporter);
        let compositor =
            Compositor::start(renderer, exporter.clone(), settings(&config), changes.notifier())?;
        let (wake, streams_changed) = (compositor.waker(), Arc::new(Notify::new()));
        let changed = streams_changed.clone();
        exporter.on_change(move || {
            wake();
            changed.notify_one();
        })?;
        let installed = wprs_format::installed(&store.data).unwrap_or_default();
        let state = State { config, config_path, config_error, installed, ..State::default() };
        let props = watch::Sender::new(state.props());
        let (jobs, thumbs, folders) = (Jobs::new(), Arc::new(Thumbs::new(&store.cache)), Folders::new());
        let state = Mutex::new(state);
        let daemon = Self {
            compositor,
            state,
            exporter,
            streams_changed,
            props,
            adapter,
            tier,
            store,
            changes,
            jobs,
            thumbs,
            folders,
            rotations: Rotations::default(),
            remote: Default::default(),
            account: Default::default(),
            turned: Notify::new(),
        };
        daemon.refresh_items();
        Ok(daemon)
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    pub(crate) fn publish(&self) {
        let st = self.state();
        self.props.send_if_modified(|props| std::mem::replace(props, st.props()) != *props);
        self.changes.changed("state", Vec::new());
    }

    pub fn retry_effects(self: &Arc<Self>) -> anyhow::Result<()> {
        crate::compositor::stops::forgive();
        self.compositor.retry_effects();
        crate::compositor::stops::recheck(self);
        Ok(())
    }

    pub fn reload(&self) -> anyhow::Result<()> {
        let settings = {
            let mut st = self.state();
            let loaded =
                load(&st.config_path, &self.store).inspect_err(|e| st.config_error = Some(e.to_string()))?;
            (st.config, st.config_error) = (loaded, None);
            st.installed = wprs_format::installed(&self.store.data).unwrap_or_default();
            settings(&st.config)
        };
        self.compositor.set_settings(settings);
        self.configure_all();
        self.refresh_items();
        self.watch_sources();
        self.rotations.wake.notify_one();
        self.remote.poke.notify_one();
        self.publish();
        Ok(())
    }
}

fn load(path: &Path, store: &Store) -> Result<Config, wprs_core::ConfigError> {
    let mut config = Config::load(path)?;
    if store.drop_dangling(&mut config) {
        config.save(path)?;
    }
    Ok(config)
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
    let (max_fps, volume, muted) = (playback.max_fps, playback.volume, playback.muted);
    let (software_parallax, flash_guard) = (playback.software_parallax, playback.flash_guard);
    Settings { transition, max_fps, volume, muted, software_parallax, flash_guard }
}

pub(crate) fn output_name(output: &str) -> fdo::Result<String> {
    let output = output.trim();
    if output.len() > 128 || output.chars().any(char::is_control) {
        return Err(fdo::Error::InvalidArgs("invalid output name".into()));
    }
    Ok(if output.is_empty() { dbus::DEFAULT_OUTPUT.to_owned() } else { output.to_owned() })
}
