use crate::pack::Pack;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use wprs_scene::{Course, Fired, LayerRead, MAX_FIRED, MAX_TOUCHES, Property, Runtime, Touch, Values, Wake};
use wprs_script::{Error, Frame, Limits, Local, Mode, Setup};

const CALM: u32 = 30;
const SETTLE: u32 = 3;
const POLL: f64 = 0.5;

static STATE: AtomicU32 = AtomicU32::new(0);
static FAILURES: AtomicU64 = AtomicU64::new(0);

pub(crate) fn scripts() -> &'static str {
    ["", "stopped", "off"][STATE.load(Relaxed).min(2) as usize]
}

pub(crate) struct Sent<'a> {
    pub(crate) properties: &'a BTreeMap<String, Property>,
    pub(crate) view: [f32; 4],
    pub(crate) screen: [u32; 2],
    pub(crate) fps: u32,
    pub(crate) quality: u8,
    pub(crate) listens: bool,
}

#[derive(Default)]
pub(crate) struct Acting {
    local: Option<Local>,
    fired: Vec<Fired>,
    courses: Vec<Course>,
    touches: Vec<Touch>,
    pub(crate) actions: Vec<String>,
    calm: u32,
    answered: bool,
    awaiting: u64,
    timer: Option<f64>,
    exposed: Option<Values>,
    props: Option<Values>,
    reads: Vec<(String, LayerRead)>,
    sounds: Vec<String>,
    filmed: Vec<Course>,
    ended: Vec<String>,
    pub(crate) keeping: crate::keeping::Keeping,
    pub(crate) moved: bool,
}

impl Acting {
    pub(crate) fn new(pack: &Pack, runtime: &Runtime) -> Acting {
        STATE.store(0, Relaxed);
        let keeping = crate::keeping::Keeping::new(pack);
        let local = (!pack.scripts.is_empty()).then(|| {
            let named = (pack.locale.as_deref().unwrap_or_default(), &pack.labels);
            let (storage, folders) = (keeping.scopes(), crate::keeping::folders(&pack.properties));
            let made = Setup::new(&pack.scene, &pack.scripts, runtime.seed, named);
            let setup = Setup { mobile: true, storage, folders, ..made };
            Local::new(&setup, Limits::default())
        });
        Acting { local, keeping, ..Acting::default() }
    }

    pub(crate) fn reprops(&mut self) {
        self.props = None;
    }

    pub(crate) fn touched(&mut self, runtime: &mut Runtime) {
        let room = MAX_TOUCHES.saturating_sub(self.touches.len());
        self.touches.extend(runtime.take_touches().into_iter().take(room));
        self.calm = 0;
    }

    pub(crate) fn step(&mut self, runtime: &mut Runtime, sent: Sent) -> bool {
        let Some(local) = self.local.as_mut() else { return false };
        let room = MAX_FIRED.saturating_sub(self.fired.len());
        self.fired.extend(runtime.take_fired().into_iter().take(room));
        let room = MAX_FIRED.saturating_sub(self.ended.len());
        self.ended.extend(runtime.take_ended().into_iter().take(room));
        runtime.fill_playbacks(&mut self.courses);
        let rt = &*runtime;
        self.keeping.list((sent.properties, &rt.scene), rt.props(), rt.time());
        rt.fill_reads(&mut self.reads);
        rt.fill_sounds(&mut self.sounds);
        rt.fill_videos(&mut self.filmed);
        let props = &*self.props.get_or_insert_with(|| wprs_scene::exposed(sent.properties, rt.props()));
        let timer = self.timer.is_some_and(|t| t <= rt.time());
        let quiet = self.fired.is_empty() && self.actions.is_empty() && self.touches.is_empty();
        let eventful = !quiet || timer || self.exposed.as_ref() != Some(props);
        let frame = Frame {
            time: rt.time(),
            dt: rt.dt(),
            wall: rt.wall(),
            zone: "",
            props,
            actions: &self.actions,
            audio: sent.listens.then(|| rt.audio()),
            pointer: rt.pointer(),
            view: sent.view,
            screen: sent.screen,
            mode: Mode::Wallpaper,
            fps: sent.fps,
            quality: sent.quality,
            timelines: &self.courses,
            fired: &self.fired,
            reads: &self.reads,
            touches: &self.touches,
            sounds: &self.sounds,
            videos: &self.filmed,
            ended: &self.ended,
            folders: &self.keeping.text,
        };
        let before = local.progress().0;
        let answer = local.update(&frame);
        if local.progress().0 > before {
            self.fired.clear();
            self.ended.clear();
            self.actions.clear();
            self.touches.clear();
            if eventful {
                (self.calm, self.awaiting) = (0, local.progress().0);
                self.exposed.clone_from(&self.props);
            }
        }
        match answer {
            Ok(Some(mut out)) => {
                self.answered = true;
                out.console.drain(..).for_each(|line| log::debug!("script {line}"));
                if !out.stopped.is_empty() && STATE.fetch_max(1, Relaxed) == 0 {
                    log::warn!("a scene script stopped: {}", out.stopped.join("; "));
                }
                self.timer = out.wake.map(|w| runtime.time() + w);
                self.moved |= self.keeping.heard(&mut out);
                let changed = runtime.set_script(out);
                self.calm = if changed { 0 } else { self.calm };
                changed
            }
            Ok(None) => false,
            Err(Error::Disabled) => {
                log::warn!("the scene scripts failed too often and are off");
                STATE.store(2, Relaxed);
                self.local = None;
                false
            }
            Err(e) => {
                if FAILURES.fetch_add(1, Relaxed) < 20 {
                    log::warn!("scene script: {e}");
                }
                false
            }
        }
    }

    pub(crate) fn wake(&mut self, frame: Wake, runtime: &Runtime) -> Wake {
        let Some(local) = &self.local else { return frame };
        let busy = runtime.script_busy();
        let timer = self.timer.map_or(Wake::Idle, |t| Wake::At((t - runtime.time()).max(0.0)));
        let wake = match () {
            _ if !self.answered || local.progress().1 < self.awaiting => Wake::Frame,
            _ if self.calm < if busy { CALM } else { SETTLE } => Wake::Frame,
            _ if busy => frame.min(Wake::At(POLL)).min(timer),
            _ => frame.min(timer),
        };
        self.calm = self.calm.saturating_add(1);
        wake
    }
}
