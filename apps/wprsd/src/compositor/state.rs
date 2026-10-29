use super::motion::Motion;
use super::output::{Output, Shown, Sink};
use super::pictures::{self, Developed, Made, Pictures};
use super::power::Power;
use super::scene::SceneState;
use super::slot::PlayerSlot;
use super::speakers::Speaker;
use super::surface::Screen;
use super::{Command, Content, OutputStatus, Picture, Reel, Settings, Spec};
use image::RgbaImage;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;
use wprs_core::Fit;
use wprs_pipewire::Exporter;
use wprs_render::Renderer;
use wprs_wayland::Window;

pub(super) struct State {
    pub(super) renderer: &'static Renderer,
    pub(super) exporter: Arc<Exporter>,
    pub(super) settings: Settings,
    pub(super) outputs: BTreeMap<String, Output>,
    pub(super) players: HashMap<Reel, PlayerSlot>,
    pub(super) tx: mpsc::Sender<Command>,
    pub(super) status: Arc<Mutex<BTreeMap<String, OutputStatus>>>,
    pub(super) on_status: Box<dyn Fn(Vec<String>) + Send>,
    pub(super) windows: HashMap<String, Window>,
    pub(super) motion: Motion,
    pub(super) power: Power,
    pub(super) speakers: BTreeMap<PathBuf, Speaker>,
    pub(super) locked: bool,
    pub(super) felt: Option<Instant>,
    pictures: Pictures,
}

impl State {
    pub(super) fn new(
        renderer: &'static Renderer,
        exporter: Arc<Exporter>,
        settings: Settings,
        tx: mpsc::Sender<Command>,
        status: Arc<Mutex<BTreeMap<String, OutputStatus>>>,
    ) -> Self {
        let (outputs, players, windows) = (BTreeMap::new(), HashMap::new(), HashMap::new());
        let (on_status, motion) = (Box::new(|_| {}), Motion::new(renderer.software()));
        let power = Power::new(renderer.tier(), false, settings.max_fps);
        let (speakers, locked) = (BTreeMap::new(), false);
        Self {
            renderer,
            exporter,
            settings,
            outputs,
            players,
            tx,
            status,
            on_status,
            windows,
            motion,
            power,
            speakers,
            locked,
            felt: None,
            pictures: Pictures::default(),
        }
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
            self.feel(Instant::now());
            self.render_due();
            self.sync_sounds();
            self.publish_status();
        }
    }

    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Configure(output, size, spec) => self.configure(output, size, spec),
            Command::Remove(output) => {
                super::feed::declare(&output, None);
                drop((self.outputs.remove(&output), self.windows.remove(&output)));
                self.pictures.prune(&self.outputs);
                self.follow(&output);
            }
            Command::Attach(output, window) => self.attach(output, window),
            Command::Frame(output) => self.frame_done(&output),
            Command::Repaint(output) => self.repaint(&output),
            Command::Backdrop(output) => self.backdrop_again(&output),
            Command::SetActive(output, active, covered) => {
                self.outputs.get_mut(&output).into_iter().for_each(|o| o.covered = covered);
                if let Some(o) = self.outputs.get_mut(&output).filter(|o| o.active != active) {
                    o.active = active;
                    o.shown.scene.iter_mut().filter(|_| !active).for_each(SceneState::pause);
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
            Command::Saver(saver) => {
                self.power.saver = saver;
                self.outputs.values_mut().filter(|o| o.shown.scene.is_some()).for_each(|o| o.dirty = true);
            }
            Command::Locked(locked) => {
                self.locked = locked;
                self.hush();
            }
            Command::Press(output, id) => {
                if let Some(o) = self.outputs.get_mut(&output)
                    && let Some(scene) = o.shown.scene.as_mut()
                {
                    scene.press(&id);
                    o.dirty = true;
                }
            }
            Command::Wake => {}
            Command::Quit => return false,
        }
        true
    }

    fn configure(&mut self, name: String, size: (u32, u32), mut spec: Spec) {
        let slot = spec.content.video().and_then(|reel| self.players.get_mut(reel));
        slot.into_iter().for_each(|slot| slot.revive(Instant::now()));
        self.outputs.get_mut(&name).into_iter().for_each(|o| o.refresh = spec.refresh);
        let resume = self.outputs.get(&name).and_then(|o| match (&o.shown.content, &spec.content) {
            (Content::Scene(old), Content::Scene(new))
                if old.pack.path == new.pack.path && new.hold.is_none() =>
            {
                o.shown.scene.as_ref().map(SceneState::time)
            }
            _ => None,
        });
        let mut current = self.outputs.get_mut(&name).filter(|o| o.size == size);
        if current.as_ref().is_some_and(|o| o.shown.matches(&spec)) {
            return;
        }
        if let Some(o) = current.as_deref_mut()
            && let (Content::Scene(old), Content::Scene(new)) = (&o.shown.content, &spec.content)
            && Arc::ptr_eq(&old.pack, &new.pack)
            && (o.shown.fit, o.shown.background) == (spec.fit, spec.background)
            && let Some(scene) = o.shown.scene.as_mut()
        {
            o.dirty |= scene.update(new.clone());
            super::feed::declare(&name, Some(scene));
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
        let (content, fit, background, transition, refresh) =
            (spec.content, spec.fit, spec.background, spec.transition, spec.refresh);
        let scene = match &content {
            Content::Scene(s) => {
                let mut scene = SceneState::new(s.clone(), self.power);
                scene.calm(self.motion.calm);
                scene.guard(self.settings.flash_guard);
                resume.into_iter().for_each(|time| scene.resume(time));
                Some(scene)
            }
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
                self.outputs.insert(name.clone(), Output::new(size, shown, sink, refresh));
            }
        }
        self.pictures.prune(&self.outputs);
        self.follow(&name);
    }

    fn upload(
        &mut self,
        output: &str,
        picture: &Picture,
        pixels: Option<Arc<RgbaImage>>,
        fit: Fit,
        size: (u32, u32),
    ) -> Option<Made> {
        let decoded = pixels.as_deref().map(RgbaImage::dimensions);
        if let Some(shared) = self.pictures.shared(self.renderer, picture, decoded, (fit, size)) {
            return Some(shared);
        }
        let made = pictures::develop(self.renderer, output, picture, pixels, (fit, size))?;
        let developed =
            Developed { look: pictures::look(picture), image: made.0.clone(), relief: made.2.clone() };
        self.pictures.keep(developed, &mut self.outputs);
        Some(made)
    }

    fn apply_settings(&mut self, settings: Settings) {
        let old = std::mem::replace(&mut self.settings, settings);
        self.power.max_fps = settings.max_fps;
        self.outputs.values_mut().for_each(|o| o.guard(settings.flash_guard));
        if old.software_parallax != settings.software_parallax && self.motion.software {
            self.restill();
        }
        self.hush();
        let mut players = self.players.values().filter(|s| s.volume > 0).filter_map(|s| s.player.as_ref());
        if !settings.muted && settings.volume > 0.0 && players.any(|p| !p.audio_enabled()) {
            self.players.retain(|_, slot| slot.volume == 0);
        }
    }

    pub(super) fn hush(&self) {
        let heard = (self.settings, self.locked);
        self.players.values().for_each(|slot| slot.hush(heard));
    }
}
