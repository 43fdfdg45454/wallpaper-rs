use super::super::Reel;
use super::super::power::Power;
use super::super::slot::PlayerSlot;
use super::{CALM, POLL, SETTLE, SceneState, after, assets};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wprs_render::{GpuImage, RenderError, Renderer};
use wprs_scene::Wake;

impl SceneState {
    pub(crate) fn render(
        &mut self,
        renderer: &Renderer,
        (size, refresh): ((u32, u32), u32),
        power: Power,
        players: &HashMap<Reel, PlayerSlot>,
        active: bool,
        now: Instant,
    ) -> Result<bool, RenderError> {
        self.due = None;
        let dt = self.step(active, now);
        let p = self.pointer;
        let pointer = self.runtime.pointer().follow(p.pos, p.buttons, dt as f32);
        self.runtime.set_pointer(pointer);
        self.runtime.set_wall(crate::zone::wall());
        let (quality, uses) = (self.level(power), self.uses);
        if quality != self.quality {
            (self.quality, self.uses) = (quality, self.needs(quality));
        }
        if let Some(view) = self.view.as_mut().filter(|_| std::mem::take(&mut self.restock)) {
            let assets = assets(&self.spec);
            renderer.refresh_scene(view, (&self.spec.pack.scene, self.runtime.props()), &assets)?;
        }
        if self.view.is_none() {
            let view =
                renderer.scene_view(&self.spec.pack.scene, &assets(&self.spec), self.runtime.props())?;
            if view.gpu_particles() == Some(false) {
                tracing::debug!("particles run on the CPU with a smaller budget");
            }
            self.view = Some(view);
        }
        let view = self.view.as_mut().expect("created above");
        let inner = view.inner(renderer, size, quality);
        let rect = view.view(inner);
        let presses = std::mem::take(&mut self.presses);
        if self.uses.cursor && self.runtime.point(view.hitboxes(), rect, &presses, self.allowed.into()) {
            self.calm = 0;
            let room = wprs_scene::MAX_TOUCHES.saturating_sub(self.touches.len());
            self.touches.extend(self.runtime.take_touches().into_iter().take(room));
        }
        let frame = self.runtime.frame_in(std::mem::take(&mut self.spare), rect, quality);
        let videos: Vec<(&str, &GpuImage)> = self
            .spec
            .reels()
            .filter_map(|(id, reel, _)| Some((id, players.get(reel)?.texture.as_ref()?)))
            .collect();
        let rendered = renderer.render_scene(view, &self.runtime, &frame, inner, quality, &videos).map(drop);
        if view.left_key() != self.left_key {
            (self.left, self.left_key) = (view.left_out(), view.left_key());
        }
        rendered?;
        self.damage = wprs_core::damage::join(self.damage, view.damage().map(|r| r.scaled(inner, size)));
        if let Some(guard) = self.guard.as_mut() {
            let image = view.image().filter(|_| view.stats().damage > 0);
            guard.watch(renderer, image, now, &self.spec.pack.props.id)?;
        }
        let (fps, timed) = (power.fps(&self.runtime, quality, refresh), view.gpu_time());
        let interval = Duration::from_secs(1) / fps;
        self.sent = (std::mem::take(&mut self.sent.0), frame.view, [size.0, size.1], fps);
        let stopped = self.guard.as_ref().is_some_and(|g| g.speed() == 0.0);
        let busy = self.runtime.script_busy();
        let timer = match self.timer - self.runtime.time() {
            left if left.is_finite() => Wake::At(left.max(0.0)),
            _ => Wake::Idle,
        };
        let wake = match &self.host {
            _ if self.still || stopped => Wake::Idle,
            Some(h) if !self.answered || h.progress().1 < self.awaiting => Wake::Frame,
            Some(_) if self.calm < if busy { CALM } else { SETTLE } => Wake::Frame,
            Some(_) if busy => frame.wake.min(Wake::At(POLL)).min(timer),
            Some(_) => frame.wake.min(timer),
            None => frame.wake,
        };
        let wake = if self.hushed { self.runtime.calmed(wake) } else { wake };
        self.spare = frame.spare();
        self.calm = self.calm.saturating_add(1);
        self.moving = wake == Wake::Frame;
        self.due = match wake {
            Wake::Frame => Some(now + interval),
            Wake::At(seconds) => Some(now + after(seconds).max(interval)),
            Wake::Idle => None,
        };
        if self.due.is_none() {
            view.trim();
        }
        self.judge(timed, power, refresh, now);
        self.listen(interval, now);
        Ok(uses != self.uses)
    }

    fn step(&mut self, active: bool, now: Instant) -> f64 {
        let dt = self.tick.filter(|_| active).map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f64());
        if active && !self.still && !self.hushed {
            self.runtime.advance(dt * self.guard.as_ref().map_or(1.0, |g| g.speed()));
        }
        self.tick = active.then_some(now);
        dt
    }

    pub(crate) fn tracks(&self) -> Vec<wprs_video::Track> {
        let sounds = self.spec.pack.scene.sounds();
        let fades = |s: &wprs_scene::Sound| (s.fade_in, s.fade_out);
        let lists = self.runtime.playlists().iter().cloned().zip(sounds.map(fades));
        lists.map(|(list, (fade_in, fade_out))| wprs_video::Track { list, fade_in, fade_out }).collect()
    }

    pub(crate) fn hold(&mut self) {
        self.tick = None;
    }

    pub(crate) fn hear(&mut self, interval: Duration, now: Instant) {
        self.step(true, now);
        self.listen(interval, now);
    }

    fn listen(&mut self, interval: Duration, now: Instant) {
        if self.spec.pack.sounds.is_empty() {
            return;
        }
        let (voices, wake) = self.runtime.voices();
        self.voices = voices;
        self.heard = match wake {
            Wake::Frame => Some(now + interval),
            Wake::At(seconds) => Some(now + after(seconds).max(interval)),
            Wake::Idle => None,
        };
    }
}
