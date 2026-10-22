use super::output::{Output, Shown};
use super::slot::PlayerSlot;
use super::{Command, Content, OutputStatus, Settings, Spec};
use image::RgbaImage;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;
use wprs_core::Fit;
use wprs_pipewire::Exporter;
use wprs_render::{GpuImage, Renderer, decode_image};

pub(super) struct State {
    pub(super) renderer: Renderer,
    pub(super) exporter: Arc<Exporter>,
    pub(super) settings: Settings,
    pub(super) outputs: BTreeMap<String, Output>,
    pub(super) players: HashMap<PathBuf, PlayerSlot>,
    pub(super) tx: mpsc::Sender<Command>,
    pub(super) status: Arc<Mutex<BTreeMap<String, OutputStatus>>>,
    pub(super) on_status: Box<dyn Fn(Vec<String>) + Send>,
}

impl State {
    pub(super) fn new(
        renderer: Renderer,
        exporter: Arc<Exporter>,
        settings: Settings,
        tx: mpsc::Sender<Command>,
        status: Arc<Mutex<BTreeMap<String, OutputStatus>>>,
    ) -> Self {
        let (outputs, players, on_status) = (BTreeMap::new(), HashMap::new(), Box::new(|_| {}));
        Self { renderer, exporter, settings, outputs, players, tx, status, on_status }
    }

    pub(super) fn run(&mut self, rx: &mpsc::Receiver<Command>) {
        loop {
            let first = match self.next_deadline() {
                Some(deadline) => match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    received => received.ok(),
                },
                None => match rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => return,
                },
            };
            for command in first.into_iter().chain(std::iter::from_fn(|| rx.try_recv().ok())) {
                if !self.handle(command) {
                    return;
                }
            }
            self.sync_players();
            self.take_video_frames();
            self.start_fades();
            self.render_due();
            self.publish_status();
        }
    }

    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Configure(output, size, spec) => self.configure(output, size, spec),
            Command::Remove(output) => drop(self.outputs.remove(&output)),
            Command::SetActive(output, active) => {
                if let Some(o) = self.outputs.get_mut(&output).filter(|o| o.active != active) {
                    o.active = active;
                    let cancelled = !active && o.fade.take().is_some();
                    o.dirty |= cancelled || (active && o.animated());
                }
            }
            Command::Settings(settings) => self.apply_settings(settings),
            Command::Wake => {}
            Command::Quit => return false,
        }
        true
    }

    fn configure(&mut self, name: String, size: (u32, u32), mut spec: Spec) {
        let slot = spec.content.video().and_then(|path| self.players.get_mut(path));
        slot.into_iter().for_each(|slot| slot.revive(Instant::now()));
        let current = self.outputs.get_mut(&name).filter(|o| o.size == size);
        if current.as_ref().is_some_and(|o| o.shown.matches(&spec)) {
            return;
        }
        let same = |o: &&mut Output| o.shown.fit == spec.fit && o.shown.content.same_source(&spec.content);
        let kept = current.filter(same).and_then(|o| o.shown.image.take());
        let image = match &mut spec.content {
            Content::Image(picture) => {
                let pixels = picture.pixels.take();
                kept.or_else(|| self.upload(&name, &picture.file, pixels, spec.fit, size))
            }
            _ => kept,
        };
        let shown = Shown { content: spec.content, fit: spec.fit, background: spec.background, image };
        let transition = !self.settings.transition.is_zero();
        match self.outputs.get_mut(&name) {
            Some(output) => output.show(shown, size, transition),
            None => drop(self.outputs.insert(name, Output::new(size, shown))),
        }
    }

    fn upload(
        &self,
        output: &str,
        file: &Path,
        pixels: Option<Arc<RgbaImage>>,
        fit: Fit,
        size: (u32, u32),
    ) -> Option<GpuImage> {
        let pixels = pixels.map_or_else(|| decode_image(file, self.renderer.max_edge()).map(Arc::new), Ok);
        let uploaded =
            pixels.map_err(anyhow::Error::from).and_then(|p| Ok(self.renderer.upload(&p, fit, size)?));
        uploaded.inspect_err(|e| tracing::warn!(output, "cannot show the image: {e}")).ok()
    }

    fn apply_settings(&mut self, settings: Settings) {
        let old = std::mem::replace(&mut self.settings, settings);
        let players: Vec<_> = self.players.values().filter_map(|s| s.player.as_ref()).collect();
        players.iter().for_each(|p| p.set_volume(settings.volume, settings.muted));
        let needs_audio =
            !settings.muted && settings.volume > 0.0 && players.iter().any(|p| !p.audio_enabled());
        if old.max_fps != settings.max_fps || needs_audio {
            self.players.clear();
        }
    }
}
