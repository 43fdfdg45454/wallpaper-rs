use super::motion::Motion;
use super::output::{Output, Raised, Shown, Sink};
use super::power::Power;
use super::scene::SceneState;
use super::slot::PlayerSlot;
use super::surface::Screen;
use super::{Command, Content, OutputStatus, Picture, Settings, Spec};
use image::RgbaImage;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;
use wprs_core::edit::Edit;
use wprs_core::{Fit, UvRect};
use wprs_pipewire::Exporter;
use wprs_render::{GpuImage, Renderer, decode_image, needed_edge};
use wprs_wayland::Window;

pub(super) struct State {
    pub(super) renderer: Renderer,
    pub(super) exporter: Arc<Exporter>,
    pub(super) settings: Settings,
    pub(super) outputs: BTreeMap<String, Output>,
    pub(super) players: HashMap<PathBuf, PlayerSlot>,
    pub(super) tx: mpsc::Sender<Command>,
    pub(super) status: Arc<Mutex<BTreeMap<String, OutputStatus>>>,
    pub(super) on_status: Box<dyn Fn(Vec<String>) + Send>,
    pub(super) windows: HashMap<String, Window>,
    pub(super) motion: Motion,
    pub(super) power: Power,
}

impl State {
    pub(super) fn new(
        renderer: Renderer,
        exporter: Arc<Exporter>,
        settings: Settings,
        tx: mpsc::Sender<Command>,
        status: Arc<Mutex<BTreeMap<String, OutputStatus>>>,
    ) -> Self {
        let (outputs, players, windows) = (BTreeMap::new(), HashMap::new(), HashMap::new());
        let (on_status, motion) = (Box::new(|_| {}), Motion::new(renderer.software()));
        let tier = wprs_scene::Tier::detect(renderer.software(), renderer.discrete());
        let power = Power { tier, on_battery: false, max_fps: settings.max_fps };
        Self { renderer, exporter, settings, outputs, players, tx, status, on_status, windows, motion, power }
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
            let batch: Vec<Command> =
                first.into_iter().chain(std::iter::from_fn(|| rx.try_recv().ok())).collect();
            let later = |i: usize, name: &String| {
                batch[i + 1..].iter().any(|c| matches!(c, Command::Configure(n, ..) if n == name))
            };
            let stale: Vec<bool> = batch
                .iter()
                .enumerate()
                .map(|(i, c)| matches!(c, Command::Configure(n, ..) if later(i, n)))
                .collect();
            for (command, stale) in batch.into_iter().zip(stale) {
                if !stale && !self.handle(command) {
                    return;
                }
            }
            self.sync_players();
            self.feed_inputs();
            self.take_video_frames();
            self.start_fades();
            self.check_starvation();
            self.move_reliefs();
            self.render_due();
            self.publish_status();
        }
    }

    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Configure(output, size, spec) => self.configure(output, size, spec),
            Command::Remove(output) => {
                super::feed::declare(&output, None);
                drop((self.outputs.remove(&output), self.windows.remove(&output)));
                self.follow(&output);
            }
            Command::Attach(output, window) => self.attach(output, window),
            Command::Frame(output) => self.frame_done(&output),
            Command::SetActive(output, active) => {
                if let Some(o) = self.outputs.get_mut(&output).filter(|o| o.active != active) {
                    o.active = active;
                    let cancelled = !active && o.fade.take().is_some();
                    o.dirty |= cancelled || (active && o.animated());
                }
            }
            Command::Settings(settings) => self.apply_settings(settings),
            Command::Calm(calm) => self.calm(calm),
            Command::Power(on_battery) => {
                self.power.on_battery = on_battery;
                self.outputs.values_mut().filter(|o| o.shown.scene.is_some()).for_each(|o| o.dirty = true);
            }
            Command::Wake => {}
            Command::Quit => return false,
        }
        true
    }

    fn configure(&mut self, name: String, size: (u32, u32), mut spec: Spec) {
        let slot = spec.content.video().and_then(|path| self.players.get_mut(path));
        slot.into_iter().for_each(|slot| slot.revive(Instant::now()));
        let mut current = self.outputs.get_mut(&name).filter(|o| o.size == size);
        if current.as_ref().is_some_and(|o| o.shown.matches(&spec)) {
            return;
        }
        if let Some(o) = current.as_deref_mut()
            && let (Content::Scene(old), Content::Scene(new)) = (&o.shown.content, &spec.content)
            && old.same_source(new)
            && (o.shown.fit, o.shown.background) == (spec.fit, spec.background)
            && let Some(scene) = o.shown.scene.as_mut()
        {
            o.dirty |= scene.update(new.clone());
            o.shown.content = spec.content;
            return;
        }
        let same = |o: &&mut Output| o.shown.fit == spec.fit && o.shown.content.same_look(&spec.content);
        let kept = current
            .filter(same)
            .and_then(|o| Some((o.shown.image.take()?, o.shown.uv?, o.shown.relief.take())));
        let (image, uv, relief) = match &mut spec.content {
            Content::Image(picture) => {
                let pixels = picture.pixels.take();
                kept.or_else(|| self.upload(&name, picture, pixels, spec.fit, size))
            }
            _ => kept,
        }
        .map_or((None, None, None), |(image, uv, relief)| (Some(image), Some(uv), relief));
        let (content, fit, background, transition) =
            (spec.content, spec.fit, spec.background, spec.transition);
        let scene = match &content {
            Content::Scene(s) => Some(SceneState::new(s.clone())),
            _ => None,
        };
        super::feed::declare(&name, scene.as_ref());
        let shown = Shown { content, fit, background, image, uv, transition, relief, scene };
        let transition = !transition.unwrap_or(self.settings.transition).is_zero();
        match self.outputs.get_mut(&name) {
            Some(output) => output.show(shown, size, transition),
            None => {
                let sink =
                    self.windows.remove(&name).map_or(Sink::Stream(None), |w| Sink::Surface(Screen::new(w)));
                self.outputs.insert(name.clone(), Output::new(size, shown, sink));
            }
        }
        self.follow(&name);
    }

    fn upload(
        &self,
        output: &str,
        picture: &Picture,
        pixels: Option<Arc<RgbaImage>>,
        fit: Fit,
        size: (u32, u32),
    ) -> Option<(GpuImage, UvRect, Option<Raised>)> {
        let (file, edit, max) = (&picture.file, picture.edit.as_deref(), self.renderer.max_edge());
        let edge = || needed_edge(file, edit, &picture.place, (fit, size), max);
        let pixels = pixels.map_or_else(|| decode_image(file, edge()).map(Arc::new), Ok);
        let depth = picture.depth.as_ref().map(|d| &*d.map);
        let shown = pixels.map_err(anyhow::Error::from).and_then(|p| {
            let (moving, edit) = (edit.and_then(Edit::moving), edit.filter(|e| e.develops()));
            let frame = edit.map_or(p.dimensions(), |e| e.geometry.frame(p.dimensions()));
            let uv = picture.place.uv(fit, frame, size);
            let image = match edit {
                Some(edit) => {
                    self.renderer.develop(&p, edit, depth, self.renderer.develop_size(frame, uv, size))?
                }
                None => self.renderer.upload_placed(&p, uv, size)?,
            };
            let relief = match (moving, depth) {
                (Some(parallax), Some(map)) if !uv.repeat => {
                    Some((self.renderer.upload_depth(map, edit)?, parallax.clone()))
                }
                _ => None,
            };
            Ok((image, uv, relief))
        });
        shown.inspect_err(|e| tracing::warn!(output, "cannot show the image: {e}")).ok()
    }

    fn apply_settings(&mut self, settings: Settings) {
        let old = std::mem::replace(&mut self.settings, settings);
        self.power.max_fps = settings.max_fps;
        if old.software_parallax != settings.software_parallax && self.motion.software {
            self.restill();
        }
        let players: Vec<_> = self.players.values().filter_map(|s| s.player.as_ref()).collect();
        players.iter().for_each(|p| p.set_volume(settings.volume, settings.muted));
        let needs_audio =
            !settings.muted && settings.volume > 0.0 && players.iter().any(|p| !p.audio_enabled());
        if old.max_fps != settings.max_fps || needs_audio {
            self.players.clear();
        }
    }
}
