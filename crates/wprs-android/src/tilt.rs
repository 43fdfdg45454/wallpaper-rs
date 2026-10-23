use crate::power::Power;
use crate::{choice, depth};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use wprs_core::edit::Parallax;
use wprs_core::motion::{Spring, Tilt, Turn, aim};

const FRESH: Duration = Duration::from_millis(500);
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

pub(crate) fn sense(reading: Reading, rotation: i32, now: Instant) -> bool {
    let mut sensed = sensed();
    let fresh = sensed.take().filter(|s| s.at.is_some_and(|at| now.saturating_duration_since(at) < FRESH));
    let restarted = fresh.is_none();
    let mut s = fresh.unwrap_or_default();
    let dt = s.at.map_or(0.0, |at| now.saturating_duration_since(at).as_secs_f32());
    s.goal = match reading {
        Reading::Gravity(up) => s.tilt.follow(screen(up, rotation), dt),
        Reading::Turn(turn) => aim(screen(s.turn.follow(turn, dt), rotation)),
    };
    s.at = Some(now);
    let wake = restarted || apart(s.goal, s.woke, f32::from_bits(REACH.load(Relaxed)).max(1.0));
    if wake {
        s.woke = s.goal;
    }
    *sensed = Some(s);
    wake
}

pub(crate) fn goal(offset: f32, now: Instant) -> [f32; 2] {
    let sensed = sensed();
    let fresh = sensed.as_ref().filter(|s| s.at.is_some_and(|at| now.saturating_duration_since(at) < FRESH));
    fresh.map_or([1.0 - 2.0 * offset.clamp(0.0, 1.0), 0.0], |s| s.goal)
}

pub(crate) fn wanted(preview: bool) -> bool {
    let shown = choice::shown(preview).filter(|shown| !shown.video && !shown.scene);
    shown.is_some_and(|shown| depth::raised(&shown.path).is_some())
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
mod tests {
    use super::*;

    fn turned(angle: f32) -> Reading {
        let (s, c) = (angle / 2.0).sin_cos();
        Reading::Turn([c, 0.0, s, 0.0])
    }

    #[test]
    fn turning_or_tilting_wakes_the_wallpaper_and_stillness_does_not() {
        assert_eq!(
            [0, 1, 2, 3].map(|r| screen([1.0, 0.0, 0.0], r)),
            [[1.0, 0.0, 0.0], [-0.0, 1.0, 0.0], [-1.0, -0.0, 0.0], [0.0, -1.0, 0.0]]
        );
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        assert!(sense(turned(0.0), 0, start));
        assert!(!sense(turned(0.0), 0, at(20)));
        assert!(sense(turned(0.1), 0, at(40)));
        let left = goal(0.5, at(50));
        assert!((left[0] + 0.4).abs() < 0.02 && left[1].abs() < 1e-3, "{left:?}");
        assert!(sense(turned(0.1), 1, at(60)));
        let sideways = goal(0.5, at(70));
        assert!(sideways[0].abs() < 1e-3 && (sideways[1] + 0.4).abs() < 0.02, "{sideways:?}");
        assert_eq!(goal(0.75, at(2000)), [-0.5, 0.0]);
        assert!(sense(Reading::Gravity([0.0, 0.8, 0.6]), 0, at(3000)), "a new reading starts over");
        assert!(sense(Reading::Gravity([0.3, 0.75, 0.6]), 0, at(3020)));
        assert!(goal(0.5, at(3030))[0] > 0.5);
    }

    #[test]
    fn the_phone_shifts_a_share_of_what_it_shows_and_zooms_to_hide_the_edges() {
        let parallax = Parallax::default();
        assert_eq!(shift([0.5, -0.5], 0.02, 0.5, 1.0), [0.03, -0.015]);
        assert_eq!(shift([0.5, 0.0], 0.02, 1.0, 0.5), [0.015, 0.0]);
        assert_eq!(pixels(&parallax, (1080, 2400)), 0.02 * 3.0 * 1080.0);
        let reach = shift([1.0, 0.0], parallax.strength, 1.0, 0.45)[0] / 0.45 * 1080.0;
        assert!((pixels(&parallax, (1080, 2400)) - reach).abs() < 1e-3, "{reach}");
        let zoom = overscan(&parallax, 0.75, 1.0);
        assert!((zoom - 1.0 / (1.0 - 2.0 * 0.042)).abs() < 1e-4, "{zoom}");
        assert_eq!(overscan(&Parallax { strength: 0.0, ..parallax.clone() }, 1.0, 1.0), 1.04);
        assert!(overscan(&Parallax { strength: 0.03, focus: 0.0, ..parallax }, 4.0, 2.0) <= 1.0 / 0.6 + 1e-4);
    }

    #[test]
    fn the_spring_settles_and_stops_asking_for_frames() {
        let mut motion = Motion::default();
        let steps: Vec<_> =
            (1..=60).map(|i| motion.step([0.5, -0.5], 150, (i * 33_000_000, 30), 144.0)).collect();
        assert!(steps[0].0 && steps[0].1);
        assert_eq!(steps.last(), Some(&(false, false)));
        assert!((motion.at()[0] - 0.5).abs() < 0.01);
        assert!(motion.rest() && !motion.rest());
    }
}
