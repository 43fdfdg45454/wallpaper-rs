use super::report::wall;
use super::{CALMING, ENGINE, LEFT, LEVEL, RETRIES, Shot, Stage};
use crate::acting::Sent;
use crate::hearing;
use crate::power::{Power, delay_ms};
use std::sync::PoisonError;
use std::sync::atomic::Ordering::Relaxed;
use wprs_render::{GpuImage, RenderError, Renderer, SceneView};
use wprs_scene::{Budget, Content, FlashGuard, Quality, Tier, Wake, Why};

const LISTEN_MS: i64 = 250;
const QUIET_MS: i64 = 2_000;
const MAX_GAP: f64 = 600.0;
const DAY_MS: f64 = 86_400_000.0;
const PHONE_SHARE: f32 = 0.25;
const SLACK_NS: i64 = 2_000_000;

impl Stage {
    pub(crate) fn render<'a>(
        &'a mut self,
        r: &Renderer,
        (size, nanos, cap): ((u32, u32), i64, u32),
        (power, tier): (Power, Tier),
        videos: &[(&str, &GpuImage)],
        force: bool,
    ) -> Result<Shot<'a>, RenderError> {
        let budget = Budget::new(tier, !power.charging, cap.max(1));
        self.runtime.set_battery(!power.charging);
        self.heavy = budget.still(&self.runtime);
        if self.heavy && !std::mem::replace(&mut self.posed, true) {
            self.runtime.seek(f64::from(self.runtime.scene.clock.poster_time));
        }
        let calm = power.still && !self.heavy && !self.moving;
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
        if std::mem::take(&mut self.acting.moved) {
            self.gather();
        }
        let asked = power.still && !self.moving;
        let stilled = std::mem::replace(&mut self.stilled, asked) != asked;
        let calmed = stilled && self.runtime.set_reduced_motion(asked);
        let tipped = self.runtime.set_tilt(crate::tilt::goal(self.offset, std::time::Instant::now()));
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
        if std::mem::replace(&mut self.quality, quality) != quality && self.runtime.set_quality(quality) {
            self.restock = true;
            self.acting.reprops();
        }
        if let Some(view) = self.view.as_mut().filter(|_| std::mem::take(&mut self.restock)) {
            r.refresh_scene(view, (&self.pack.scene, self.runtime.props()), &self.assets)?;
        }
        if self.view.is_none() {
            self.view = Some(r.scene_view(&self.pack.scene, &self.assets, self.runtime.props())?);
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
        let rendered =
            crate::counted::gpu(|| r.render_scene(view, &self.runtime, &frame, inner, quality, videos))
                .map(drop);
        self.spare = frame.spare();
        let (samples, gpu_ms) = view.gpu_time();
        if std::mem::replace(&mut self.samples, samples) != samples && !still {
            let runtime = &self.runtime;
            let budget_ms =
                |q| (1000.0 / budget.fps(runtime, q, 0).max(1) as f32 * PHONE_SHARE, runtime.cost(q));
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
}
