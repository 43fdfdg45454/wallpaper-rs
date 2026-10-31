use crate::power::Power;
use crate::{choice, depth};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use wprs_core::edit::Parallax;
use wprs_core::motion::{Spring, Tilt, Turn, aim};

const FRESH: Duration = Duration::from_millis(500);
const SILENCE: Duration = Duration::from_secs(2);
const SUBPIXEL: f32 = 0.25;
pub(crate) const PHONE: f32 = 3.0;
const MARGIN: f32 = 0.2;

static SENSED: Mutex<Option<Sensed>> = Mutex::new(None);
static REACH: AtomicU32 = AtomicU32::new(0);

pub(crate) enum Reading {
    Gravity([f32; 3]),
    Turn([f32; 4]),
}

#[derive(Default)]
struct Sensed {
    tilt: Tilt,
    turn: Turn,
    at: Option<Instant>,
    goal: [f32; 2],
    woke: [f32; 2],
}

#[derive(Default)]
pub(crate) struct Motion {
    spring: Spring,
    shown: [f32; 2],
    last: Option<i64>,
}

fn sensed() -> MutexGuard<'static, Option<Sensed>> {
    SENSED.lock().unwrap_or_else(PoisonError::into_inner)
}

fn apart(a: [f32; 2], b: [f32; 2], pixels: f32) -> bool {
    (0..2).any(|i| (a[i] - b[i]).abs() * pixels >= SUBPIXEL)
}

pub(crate) fn screen([x, y, z]: [f32; 3], rotation: i32) -> [f32; 3] {
    match rotation.rem_euclid(4) {
        1 => [-y, x, z],
        2 => [-x, -y, z],
        3 => [y, -x, z],
        _ => [x, y, z],
    }
}

impl Sensed {
    fn since(&self, now: Instant) -> Option<Duration> {
        self.at.map(|at| now.saturating_duration_since(at))
    }

    fn heard(&self, now: Instant) -> bool {
        self.since(now).is_some_and(|gap| gap < SILENCE)
    }
}

pub(crate) fn sense(reading: Reading, rotation: i32, now: Instant) -> bool {
    feel(&mut sensed(), reading, rotation, now, f32::from_bits(REACH.load(Relaxed)))
}

fn feel(sensed: &mut Option<Sensed>, reading: Reading, rotation: i32, now: Instant, reach: f32) -> bool {
    let kept = sensed.take().filter(|s| s.heard(now));
    let restarted = kept.is_none();
    let mut s = kept.unwrap_or_default();
    let dt = s.since(now).filter(|gap| *gap < FRESH).map_or(0.0, |gap| gap.as_secs_f32());
    s.goal = match reading {
        Reading::Gravity(up) => s.tilt.follow(screen(up, rotation), dt),
        Reading::Turn(turn) => aim(screen(s.turn.follow(turn, dt), rotation)),
    };
    s.at = Some(now);
    let wake = restarted || apart(s.goal, s.woke, reach.max(1.0));
    if wake {
        s.woke = s.goal;
    }
    *sensed = Some(s);
    wake
}

pub(crate) fn goal(offset: f32, now: Instant) -> [f32; 2] {
    aimed(sensed().as_ref(), offset, now)
}

fn aimed(sensed: Option<&Sensed>, offset: f32, now: Instant) -> [f32; 2] {
    sensed.filter(|s| s.heard(now)).map_or([1.0 - 2.0 * offset.clamp(0.0, 1.0), 0.0], |s| s.goal)
}

pub(crate) fn wanted(preview: bool, lock: bool) -> bool {
    choice::shown_on(preview, lock).is_some_and(|shown| match shown.scene {
        true => crate::pack::load(&shown.path).is_ok_and(|pack| pack.scene.tilts()),
        false => !shown.video && depth::raised(&shown.path).is_some(),
    })
}

pub(crate) fn scene(preview: bool, lock: bool) -> bool {
    choice::shown_on(preview, lock).is_some_and(|shown| shown.scene)
}

pub(crate) fn rate(wanted: bool, power: Power, preview: bool) -> u32 {
    if wanted && power.moving() { power.fps(preview) } else { 0 }
}

pub(crate) fn pixels(parallax: &Parallax, size: (u32, u32)) -> f32 {
    parallax.strength * PHONE * size.0 as f32
}

pub(crate) fn shift(at: [f32; 2], strength: f32, aspect: f32, visible: f32) -> [f32; 2] {
    let reach = at[0].hypot(at[1]).max(1.0);
    let s = strength * PHONE * visible.clamp(0.0, 2.0);
    [s * at[0] / reach, s * at[1] * aspect / reach]
}

pub(crate) fn overscan(parallax: &Parallax, aspect: f32, visible: f32) -> f32 {
    let lean = parallax.focus.max(1.0 - parallax.focus);
    let margin = (parallax.strength * PHONE * visible.clamp(0.0, 2.0) * aspect.max(1.0) * lean).min(MARGIN);
    parallax.overscan.max(1.0 / (1.0 - 2.0 * margin))
}

impl Motion {
    pub(crate) fn step(
        &mut self,
        goal: [f32; 2],
        smoothing_ms: u32,
        (nanos, fps): (i64, u32),
        pixels: f32,
    ) -> (bool, bool) {
        let frame = 1.0 / fps.max(1) as f32;
        let dt = self.last.map_or(frame, |last| ((nanos - last) as f32 / 1e9).clamp(0.0, frame));
        self.last = Some(nanos);
        REACH.fetch_max(pixels.max(0.0).to_bits(), Relaxed);
        self.spring.step(goal, smoothing_ms as f32 / 1000.0, dt);
        let moved = apart(self.spring.at, self.shown, pixels);
        if moved {
            self.shown = self.spring.at;
        }
        (moved, !self.spring.settled(goal))
    }

    pub(crate) fn rest(&mut self) -> bool {
        let moved = self.shown != [0.0; 2];
        *self = Self::default();
        moved
    }

    pub(crate) fn at(&self) -> [f32; 2] {
        self.shown
    }
}

#[cfg(test)]
mod tests;
