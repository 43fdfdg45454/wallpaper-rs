use crate::compositor::{Compositor, Settings, lock};
use anyhow::Context;
use image::RgbaImage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;
use wprs_core::{Config, dbus};
use wprs_pipewire::{Exporter, Reply};
use wprs_render::Renderer;

pub(crate) struct OutputInfo {
    pub(crate) size: (u32, u32),
    pub(crate) covered: bool,
}

pub(crate) struct State {
    pub(crate) config: Config,
    pub(crate) config_path: PathBuf,
    pub(crate) outputs: HashMap<String, OutputInfo>,
    pub(crate) images: HashMap<PathBuf, (Option<SystemTime>, Arc<RgbaImage>)>,
    pub(crate) user_paused: bool,
    pub(crate) on_battery: bool,
    pub(crate) locked: bool,
}

pub struct Daemon {
    pub(crate) compositor: Compositor,
    pub(crate) state: Mutex<State>,
    exporter: Arc<Exporter>,
    pub(crate) adapter: String,
    max_edge: u32,
}

impl Daemon {
    pub fn new(config: Config, config_path: PathBuf) -> anyhow::Result<Self> {
        let renderer = Renderer::new().context("cannot initialize the GPU renderer")?;
        let (adapter, max_edge) = (renderer.adapter().to_owned(), renderer.max_edge());
        tracing::info!(adapter, "renderer ready");
        let exporter = Arc::new(Exporter::start().context("cannot connect to PipeWire")?);
        let compositor = Compositor::start(renderer, exporter.clone(), settings(&config))?;
        let state = State {
            config,
            config_path,
            outputs: HashMap::new(),
            images: HashMap::new(),
            user_paused: false,
            on_battery: false,
            locked: false,
        };
        Ok(Self { compositor, state: Mutex::new(state), exporter, adapter, max_edge })
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    pub fn check_request(&self, output: &str, width: u32, height: u32) -> Result<String, String> {
        let max = self.max_edge.min(dbus::MAX_STREAM_EDGE);
        let valid = (1..=max).contains(&width) && (1..=max).contains(&height);
        valid.then_some(()).ok_or(format!("invalid size {width}x{height} (allowed 1..={max})"))?;
        output_name(output)
    }

    pub fn ensure_stream(&self, output: &str, width: u32, height: u32, reply: Reply) {
        let info = OutputInfo { size: (width, height), covered: false };
        self.state().outputs.entry(output.to_owned()).and_modify(|o| o.size = info.size).or_insert(info);
        self.exporter.ensure_stream(output, width, height, reply);
    }

    pub fn release_stream(&self, output: &str) -> anyhow::Result<()> {
        if self.state().outputs.remove(output).is_some() {
            self.compositor.remove(output);
            self.exporter.remove_stream(output)?;
            tracing::info!(output, "stream released");
        }
        Ok(())
    }

    pub(crate) fn apply_activity(&self) {
        let st = self.state();
        st.outputs.iter().for_each(|(name, info)| self.compositor.set_active(name, st.active(info)));
    }

    pub fn reload(&self) -> anyhow::Result<()> {
        let settings = {
            let mut st = self.state();
            st.config = Config::load(&st.config_path)?;
            st.images.clear();
            settings(&st.config)
        };
        self.compositor.set_settings(settings);
        self.configure_all();
        Ok(())
    }
}

pub(crate) fn settings(config: &Config) -> Settings {
    let playback = config.playback.sanitized();
    let transition = config.transition.duration();
    Settings { transition, max_fps: playback.max_fps, volume: playback.volume, muted: playback.muted }
}

pub(crate) fn output_name(output: &str) -> Result<String, String> {
    let output = output.trim();
    if output.len() > 128 || output.chars().any(char::is_control) {
        return Err("invalid output name".into());
    }
    Ok(if output.is_empty() { dbus::DEFAULT_OUTPUT.to_owned() } else { output.to_owned() })
}
