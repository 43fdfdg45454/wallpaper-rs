use crate::acting::{Acting, Sent};
use crate::pack::Pack;
use crate::power::{Power, delay_ms};
use crate::{hearing, props, saved};
use chrono::Timelike;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};
use wprs_render::{GpuImage, Meter, RenderError, Renderer, SceneView};
use wprs_scene::{
    Budget, Content, FlashGuard, Governor, MAX_PRESSES, Press, Quality, Runtime, Spare, Tier, Wake, Why,
};

const SLACK_NS: i64 = 2_000_000;
const LISTEN_MS: i64 = 250;
const QUIET_MS: i64 = 2_000;
const MAX_GAP: f64 = 600.0;
const DAY_MS: f64 = 86_400_000.0;
const PHONE_SHARE: f32 = 0.25;

static LEVEL: AtomicU32 = AtomicU32::new(0);
static CALMING: AtomicU32 = AtomicU32::new(0);
static ENGINE: AtomicU32 = AtomicU32::new(0);
static LEFT: Mutex<(u64, Vec<(String, String)>)> = Mutex::new((0, Vec::new()));
static RETRIES: AtomicU64 = AtomicU64::new(0);

pub(crate) fn retry() {
    wprs_render::retry_effects();
    if let Err(e) = wprs_scene::forgive_strikes() {
        log::warn!("cannot forget the effects that stopped the GPU or the app: {e}");
    }
    match crate::library::with(|lib| Ok(lib.recheck(wprs_scene::STOPPED)?)) {
        Ok(uids) if uids.is_empty() => {}
        Ok(_) => crate::store::changed(),
        Err(e) => log::warn!("cannot check the left-out packages again: {e}"),
    }
    RETRIES.fetch_add(1, Relaxed);
}

pub(crate) fn finish() {
    if !wprs_render::stop_builds() {
        log::warn!("a package shader is still building while the wallpaper closes");
    }
    wprs_scene::end_trials();
}

pub(crate) fn flashes() -> &'static str {
    ["", "slowed", "paused"][CALMING.load(Relaxed).min(2) as usize]
}

pub(crate) fn particles() -> &'static str {
    ["", "gpu", "cpu"][ENGINE.load(Relaxed).min(2) as usize]
}

pub(crate) fn left_out() -> Vec<serde_json::Value> {
    let left = LEFT.lock().unwrap_or_else(PoisonError::into_inner);
    left.1.iter().map(|(id, reason)| serde_json::json!({ "id": id, "reason": reason })).collect()
}

type Guarded = (FlashGuard, Vec<f32>, i64, Option<Meter>);

pub(crate) struct Stage {
    pub(crate) uid: String,
    pub(crate) pack: Arc<Pack>,
    runtime: Runtime,
    view: Option<SceneView>,
    granted: [bool; 3],
    acting: Acting,
    presses: Vec<Press>,
    rect: [f32; 4],
    posed: bool,
    heavy: bool,
    quality: Quality,
    fps: u32,
    tick: Option<i64>,
    due: Option<i64>,
    touch: (Option<[f32; 2]>, u8, bool),
    restock: bool,
    calmed: bool,
    guard: Option<Box<Guarded>>,
    retried: u64,
    spare: Spare,
    governor: Governor,
    samples: u64,
}

pub(crate) struct Shot<'a> {
    pub(crate) image: Option<&'a GpuImage>,
    pub(crate) fps: u32,
    pub(crate) delay: i64,
}

pub(crate) fn level() -> Option<[String; 3]> {
    let level = LEVEL.load(Relaxed).checked_sub(1)?;
    let why = Why::ALL.get(level as usize % 8)?.name();
    Some([format!("T{}", level / 32), format!("Q{}", level / 8 % 4), why.into()])
}

pub(crate) fn wall() -> f64 {
    let now = chrono::Local::now();
    now.naive_local().and_utc().timestamp() as f64 + f64::from(now.nanosecond() % 1_000_000_000) / 1e9
}

impl Stage {
    pub(crate) fn new(uid: String, pack: Arc<Pack>) -> Stage {
        let mut runtime = Runtime::new(Arc::clone(&pack.scene), props::values(&pack));
        pack.inputs.into_iter().for_each(|allowed| runtime.declare(allowed));
        runtime.set_language(pack.locale.as_deref().unwrap_or_default());
        runtime.set_listeners(true, false);
        pack.assets.sheets.iter().for_each(|(id, d)| runtime.set_sheet(id, d.clone()));
        let granted = props::granted(&pack.id);
        let (view, quality, tick, due, touch) = (None, Quality::Q3, None, None, (None, 0, false));
        let acting = Acting::new(&pack, &runtime);
        Stage {
            uid,
            pack,
            runtime,
            view,
            granted,
            acting,
            presses: Vec::new(),
            rect: [0.0; 4],
            posed: false,
            heavy: false,
            quality,
            fps: 0,
            tick,
            due,
            touch,
            restock: false,
            calmed: false,
            guard: None,
            retried: RETRIES.load(Relaxed),
            spare: Spare::default(),
            governor: Governor::default(),
            samples: 0,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.granted = props::granted(&self.pack.id);
        let changed = self.runtime.set_props(props::values(&self.pack));
        if changed {
            self.acting.reprops();
        }
        self.restock |= changed;
        let pressed = saved::pressed(&self.pack.id);
        pressed.iter().for_each(|key| self.runtime.press(key));
        self.acting.actions.extend(pressed.iter().cloned());
        self.touch.2 |= changed || !pressed.is_empty();
    }

    #[cfg(test)]
    pub(crate) fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub(crate) fn wants(&self) -> [bool; 2] {
        let uses = self.runtime.uses_at(self.quality);
        let (moves, taps) = self.allowed();
        [uses.audio && self.granted[0], moves || taps]
    }

    fn allowed(&self) -> (bool, bool) {
        let uses = self.runtime.uses_at(self.quality);
        (uses.cursor && self.granted[1], uses.clicks && self.granted[2])
    }

    pub(crate) fn plays(&self) -> bool {
        !self.heavy && !self.pack.videos.is_empty()
    }

    pub(crate) fn touch(&mut self, at: [f32; 2], down: bool) -> bool {
        let (moves, taps) = self.allowed();
        let buttons = u8::from(down);
        if taps && buttons != self.touch.1 && self.presses.len() < MAX_PRESSES {
            self.presses.push(Press { at: Some(at), buttons });
        }
        if moves || taps {
            self.touch = (down.then_some(at).filter(|_| moves), buttons, true);
        }
        moves || taps
    }

    pub(crate) fn hide(&mut self) {
        self.tick = None;
        if let Some(view) = self.view.as_mut() {
            view.trim();
            self.runtime.restart_particles();
        }
    }

    pub(crate) fn drop_view(&mut self) {
        self.view = None;
    }

    pub(crate) fn render<'a>(
        &'a mut self,
        r: &Renderer,
        (size, nanos, cap): ((u32, u32), i64, u32),
        (power, tier): (Power, Tier),
        videos: &[(&str, &GpuImage)],
        force: bool,
    ) -> Result<Shot<'a>, RenderError> {
        let budget = Budget::new(tier, !power.charging, cap.max(1));
        self.heavy = budget.still(&self.runtime);
        if self.heavy && !std::mem::replace(&mut self.posed, true) {
            self.runtime.seek(f64::from(self.runtime.scene.clock.poster_time));
        }
        let calm = power.still && !self.heavy;
        if std::mem::replace(&mut self.calmed, calm) != calm && calm {
            self.runtime.seek(f64::from(self.runtime.scene.clock.poster_time));
        }
        if crate::power::guarded() != self.guard.is_some() {
            self.guard =
                crate::power::guarded().then(|| Box::new((FlashGuard::default(), Vec::new(), nanos, None)));
            CALMING.store(0, Relaxed);
        }
        let speed = self.guard.as_ref().map_or(1.0, |g| g.0.speed());
        let still = cap == 0 || self.heavy || calm || speed == 0.0;
        let dt = self.tick.filter(|_| !still).map_or(0.0, |t| ((nanos - t) as f64 / 1e9).clamp(0.0, MAX_GAP));
        self.tick = (!still).then_some(nanos);
        self.runtime.advance(dt * speed);
        let [listens, feels] = self.wants();
        let allowed = self.allowed();
        let listens = listens && power.moving() && !still;
        let hears = listens && hearing::live();
        let idle = match () {
            _ if hears => LISTEN_MS,
            _ if listens && hearing::playing() => QUIET_MS,
            _ => 0,
        };
        let heard = hears && self.runtime.set_audio(hearing::frame());
        let buttons = if allowed.0 && allowed.1 { self.touch.1 } else { 0 };
        let pointer = self.runtime.pointer().follow(self.touch.0, buttons, dt as f32);
        let felt = feels && self.runtime.set_pointer(pointer);
        let sent = Sent {
            properties: &self.pack.properties,
            view: self.rect,
            screen: [size.0, size.1],
            fps: self.fps,
            quality: self.quality as u8,
            listens: hears,
        };
        let acted = !still && self.acting.step(&mut self.runtime, sent);
        let calmed = self.runtime.set_reduced_motion(power.still);
        let tipped = self.runtime.set_tilt(crate::tilt::goal(0.5, std::time::Instant::now()));
        self.runtime.set_wall(wall());
        let (ceiling, why) = budget.level(&self.runtime);
        let ceiling = if power.ceiling() < ceiling { (power.ceiling(), Why::Heat) } else { (ceiling, why) };
        let (quality, why) = self.governor.level(ceiling);
        LEVEL.store(1 + (tier as u32 * 4 + quality as u32) * 8 + why as u32, Relaxed);
        let fps = if still { 0 } else { budget.fps(&self.runtime, quality, 0) };
        let retried = RETRIES.load(Relaxed);
        if std::mem::replace(&mut self.retried, retried) != retried {
            self.touch.2 = true;
            if self.view.as_mut().is_some_and(SceneView::retry) {
                self.view = None;
            }
        }
        let due = self.due.is_some_and(|due| nanos >= due - SLACK_NS) || std::mem::take(&mut self.touch.2);
        if !(force
            || due
            || heard
            || felt
            || acted
            || tipped
            || calmed
            || quality != self.quality
            || self.view.is_none())
        {
            let left = self.due.map(|due| ((due - nanos) / 1_000_000).max(1));
            let delay = left.unwrap_or(idle);
            return Ok(Shot { image: None, fps: self.fps, delay });
        }
        self.quality = quality;
        if let Some(view) = self.view.as_mut().filter(|_| std::mem::take(&mut self.restock)) {
            r.refresh_scene(view, (&self.pack.scene, self.runtime.props()), &self.pack.assets)?;
        }
        if self.view.is_none() {
            self.view = Some(r.scene_view(&self.pack.scene, &self.pack.assets, self.runtime.props())?);
        }
        let view = self.view.as_mut().expect("created above");
        let inner = view.inner(r, size, quality);
        self.rect = view.view(inner);
        let presses = std::mem::take(&mut self.presses);
        if self.runtime.point(view.hitboxes(), self.rect, &presses, allowed) {
            self.acting.touched(&mut self.runtime);
        }
        let frame = self.runtime.frame_in(std::mem::take(&mut self.spare), self.rect, quality);
        let playing =
            power.moving() && frame.draws.iter().any(|d| matches!(d.content, Content::Video { .. }));
        let wake = match frame.wake {
            wake if calm => self.runtime.calmed(wake),
            _ if still => Wake::Idle,
            _ if playing => Wake::Frame,
            wake => self.acting.wake(wake, &self.runtime),
        };
        let interval = delay_ms(fps);
        let delay = match wake {
            Wake::Frame => interval,
            Wake::At(seconds) => ((seconds * 1000.0).clamp(0.0, DAY_MS) as i64).max(interval),
            Wake::Idle => idle,
        };
        self.due = (delay > 0 && wake != Wake::Idle).then_some(nanos + delay * 1_000_000);
        self.fps = if wake == Wake::Frame { fps } else { 0 };
        let rendered = r.render_scene(view, &self.runtime, &frame, inner, quality, videos).map(drop);
        self.spare = frame.spare();
        let (samples, gpu_ms) = view.gpu_time();
        if std::mem::replace(&mut self.samples, samples) != samples && !still {
            let runtime = &self.runtime;
            let budget_ms = |q| 1000.0 / budget.fps(runtime, q, 0).max(1) as f32 * PHONE_SHARE;
            let raises = power.charging && power.ceiling() == Quality::Q3;
            self.governor.feed(std::time::Instant::now(), gpu_ms as f32, budget_ms, ceiling.0, raises);
        }
        ENGINE.store(view.gpu_particles().map_or(0, |gpu| if gpu { 1 } else { 2 }), Relaxed);
        let mut left = LEFT.lock().unwrap_or_else(PoisonError::into_inner);
        if left.0 != view.left_key() {
            *left = (view.left_key(), view.left());
        }
        drop(left);
        rendered?;
        let view: &'a SceneView = view;
        let image = view.image().expect("rendered above");
        if let Some((guard, luma, start, meter)) = self.guard.as_deref_mut() {
            let changed = (view.stats().damage > 0).then_some(image);
            if r.luma(meter.get_or_insert_with(|| r.meter()), changed, luma)? {
                let calming = match guard.watch((nanos - *start) as f64 / 1e9, luma) {
                    wprs_scene::Calming::Steady => 0,
                    wprs_scene::Calming::Slowed(_) => 1,
                    wprs_scene::Calming::Paused => 2,
                };
                CALMING.store(calming, Relaxed);
            }
        }
        Ok(Shot { image: Some(image), fps: self.fps, delay })
    }

    pub(crate) fn rest(&mut self) {
        if self.due.is_none() {
            self.view.iter_mut().for_each(SceneView::trim);
        }
    }
}
