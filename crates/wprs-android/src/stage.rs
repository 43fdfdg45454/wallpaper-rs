use crate::pack::Pack;
use crate::power::{Power, delay_ms};
use crate::{hearing, props};
use chrono::Timelike;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use wprs_render::{GpuImage, RenderError, Renderer, SceneView};
use wprs_scene::{Budget, Content, Quality, Runtime, Tier, Wake};

const SLACK_NS: i64 = 2_000_000;
const LISTEN_MS: i64 = 250;
const QUIET_MS: i64 = 2_000;
const MAX_STEP: f64 = 0.25;
const DAY_MS: f64 = 86_400_000.0;

static LEVEL: AtomicU32 = AtomicU32::new(0);

pub(crate) struct Stage {
    pub(crate) uid: String,
    pub(crate) pack: Arc<Pack>,
    runtime: Runtime,
    view: Option<SceneView>,
    granted: [bool; 2],
    posed: bool,
    heavy: bool,
    quality: Quality,
    fps: u32,
    tick: Option<i64>,
    due: Option<i64>,
    touch: (Option<[f32; 2]>, u8, bool),
}

pub(crate) struct Shot<'a> {
    pub(crate) image: Option<&'a GpuImage>,
    pub(crate) fps: u32,
    pub(crate) delay: i64,
}

pub(crate) fn level() -> Option<[String; 2]> {
    let level = LEVEL.load(Relaxed).checked_sub(1)?;
    Some([format!("T{}", level / 4), format!("Q{}", level % 4)])
}

fn wall() -> f64 {
    let now = chrono::Local::now();
    f64::from(now.num_seconds_from_midnight()) + f64::from(now.nanosecond() % 1_000_000_000) / 1e9
}

impl Stage {
    pub(crate) fn new(uid: String, pack: Arc<Pack>) -> Stage {
        let runtime = Runtime::new(Arc::clone(&pack.scene), props::values(&pack));
        let granted = props::granted(&pack.id);
        let (view, quality, tick, due, touch) = (None, Quality::Q3, None, None, (None, 0, false));
        Stage {
            uid,
            pack,
            runtime,
            view,
            granted,
            posed: false,
            heavy: false,
            quality,
            fps: 0,
            tick,
            due,
            touch,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.granted = props::granted(&self.pack.id);
        let changed = self.runtime.set_props(props::values(&self.pack));
        self.touch.2 |= changed;
    }

    pub(crate) fn wants(&self) -> [bool; 2] {
        let uses = self.runtime.uses_at(self.quality);
        [uses.audio && self.granted[0], uses.cursor && self.granted[1]]
    }

    pub(crate) fn plays(&self) -> bool {
        !self.heavy && !self.pack.videos.is_empty()
    }

    pub(crate) fn touch(&mut self, at: Option<[f32; 2]>) -> bool {
        let wanted = self.wants()[1];
        if wanted {
            self.touch = (at, u8::from(at.is_some()), true);
        }
        wanted
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
        let budget = Budget { tier, on_battery: !power.charging, max_fps: cap.max(1) };
        self.heavy = budget.still(&self.runtime);
        if self.heavy && !std::mem::replace(&mut self.posed, true) {
            self.runtime.seek(f64::from(self.runtime.scene.clock.poster_time));
        }
        let still = cap == 0 || self.heavy;
        let dt =
            self.tick.filter(|_| !still).map_or(0.0, |t| ((nanos - t) as f64 / 1e9).clamp(0.0, MAX_STEP));
        self.tick = (!still).then_some(nanos);
        self.runtime.advance(dt);
        let [listens, feels] = self.wants();
        let listens = listens && power.moving() && !still;
        let hears = listens && hearing::live();
        let idle = match () {
            _ if hears => LISTEN_MS,
            _ if listens && hearing::playing() => QUIET_MS,
            _ => 0,
        };
        let heard = hears && self.runtime.set_audio(hearing::frame());
        let pointer = self.runtime.pointer().follow(self.touch.0, self.touch.1, dt as f32);
        let felt = feels && self.runtime.set_pointer(pointer);
        self.runtime.set_wall(wall());
        let quality = budget.quality(&self.runtime).min(power.ceiling());
        LEVEL.store(1 + tier as u32 * 4 + quality as u32, Relaxed);
        let fps = if still { 0 } else { budget.fps(&self.runtime, quality, 0) };
        let due = self.due.is_some_and(|due| nanos >= due - SLACK_NS) || std::mem::take(&mut self.touch.2);
        if !(force || due || heard || felt || quality != self.quality || self.view.is_none()) {
            let left = self.due.map(|due| ((due - nanos) / 1_000_000).max(1));
            let delay = left.unwrap_or(idle);
            return Ok(Shot { image: None, fps: self.fps, delay });
        }
        self.quality = quality;
        let scale = quality.render_scale();
        let inner = [size.0, size.1].map(|v| (v as f32 * scale).round().max(1.0) as u32);
        if self.view.is_none() {
            self.view = Some(r.scene_view(&self.pack.scene, &self.pack.assets)?);
        }
        let view = self.view.as_mut().expect("created above");
        let frame = self.runtime.frame(view.view((inner[0], inner[1])), quality);
        let playing =
            power.moving() && frame.draws.iter().any(|d| matches!(d.content, Content::Video { .. }));
        let wake = match frame.wake {
            _ if still => Wake::Idle,
            _ if playing => Wake::Frame,
            wake => wake,
        };
        let interval = delay_ms(fps);
        let delay = match wake {
            Wake::Frame => interval,
            Wake::At(seconds) => ((seconds * 1000.0).clamp(0.0, DAY_MS) as i64).max(interval),
            Wake::Idle => idle,
        };
        self.due = (delay > 0 && wake != Wake::Idle).then_some(nanos + delay * 1_000_000);
        self.fps = if wake == Wake::Frame { fps } else { 0 };
        let image = r.render_scene(view, &self.runtime, &frame, (inner[0], inner[1]), quality, videos)?;
        Ok(Shot { image: Some(image), fps: self.fps, delay })
    }

    pub(crate) fn rest(&mut self) {
        if self.due.is_none() {
            self.view.iter_mut().for_each(SceneView::trim);
        }
    }
}
