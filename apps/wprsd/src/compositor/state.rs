use super::motion::Motion;
use super::output::Output;
use super::pictures::Pictures;
use super::power::Power;
use super::slot::PlayerSlot;
use super::speakers::Speaker;
use super::{Command, OutputStatus, Reel, Settings};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;
use wprs_pipewire::Exporter;
use wprs_render::Renderer;
use wprs_wayland::Window;

mod configure;
mod handle;

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
    scripts: super::scripts::Scripts,
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
            scripts: Default::default(),
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
}
