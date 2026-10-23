use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use wprs_core::motion::{Spring, Tilt};

const FRESH: Duration = Duration::from_millis(500);
const SUBPIXEL: f32 = 0.25;

static SENSED: Mutex<Option<Sensed>> = Mutex::new(None);
static REACH: AtomicU32 = AtomicU32::new(0);

struct Sensed {
    tilt: Tilt,
    at: Instant,
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

pub(crate) fn sense(up: [f32; 3], rotation: i32, now: Instant) -> bool {
    let mut sensed = sensed();
    let fresh = sensed.take().filter(|s| now.saturating_duration_since(s.at) < FRESH);
    let restarted = fresh.is_none();
    let blank = || Sensed { tilt: Tilt::default(), at: now, goal: [0.0; 2], woke: [0.0; 2] };
    let mut s = fresh.unwrap_or_else(blank);
    s.goal = s.tilt.follow(screen(up, rotation), now.saturating_duration_since(s.at).as_secs_f32());
    s.at = now;
    let wake = restarted || apart(s.goal, s.woke, f32::from_bits(REACH.load(Relaxed)));
    if wake {
        s.woke = s.goal;
    }
    *sensed = Some(s);
    wake
}

pub(crate) fn goal(offset: f32, now: Instant) -> [f32; 2] {
    let sensed = sensed();
    let fresh = sensed.as_ref().filter(|s| now.saturating_duration_since(s.at) < FRESH);
    fresh.map_or([1.0 - 2.0 * offset.clamp(0.0, 1.0), 0.0], |s| s.goal)
}

pub(crate) fn shift(at: [f32; 2], strength: f32, aspect: f32) -> [f32; 2] {
    let reach = at[0].hypot(at[1]).max(1.0);
    [strength * at[0] / reach, strength * at[1] * aspect / reach]
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

    #[test]
    fn the_screen_turns_with_the_display() {
        assert_eq!(
            [0, 1, 2, 3].map(|r| screen([1.0, 0.0, 0.0], r)),
            [[1.0, 0.0, 0.0], [-0.0, 1.0, 0.0], [-1.0, -0.0, 0.0], [0.0, -1.0, 0.0]]
        );
    }

    #[test]
    fn tilting_wakes_the_wallpaper_and_stillness_does_not() {
        let mut motion = Motion::default();
        assert_eq!(motion.step([1.0, 0.0], 100, (16_000_000, 60), 48.0), (true, true));
        let start = Instant::now();
        let upright = [0.0, 0.8, 0.6];
        assert!(sense(upright, 0, start));
        assert!(!sense(upright, 0, start + Duration::from_millis(20)));
        assert!(sense([0.3, 0.75, 0.6], 0, start + Duration::from_millis(40)));
        let tilted = goal(0.5, start + Duration::from_millis(50));
        assert!(tilted[0] > 0.5, "{tilted:?}");
        assert_eq!(goal(0.75, start + Duration::from_secs(2)), [-0.5, 0.0]);
        assert!(sense(upright, 1, start + Duration::from_secs(3)), "a new reading starts over");
    }

    #[test]
    fn the_spring_settles_and_stops_asking_for_frames() {
        let mut motion = Motion::default();
        let steps: Vec<_> =
            (1..=60).map(|i| motion.step([0.5, -0.5], 150, (i * 33_000_000, 30), 48.0)).collect();
        assert!(steps[0].0 && steps[0].1);
        assert_eq!(steps.last(), Some(&(false, false)));
        assert!((motion.at()[0] - 0.5).abs() < 0.01);
        assert_eq!(shift([0.5, -0.5], 0.02, 0.5), [0.01, -0.005]);
        assert!(motion.rest() && !motion.rest());
    }
}
