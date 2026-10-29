use std::collections::VecDeque;

pub const FLASH_GRID: [u32; 2] = [32, 18];
pub const SAFE_FLASHES: f32 = 3.0;
const CELLS: usize = (FLASH_GRID[0] * FLASH_GRID[1]) as usize;
const CHANGE: f32 = 0.1;
const DARK: f32 = 0.8;
const AREA: usize = CELLS / 36;
const SLOWEST: f64 = 0.25;
const WINDOW: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Calming {
    Steady,
    Slowed(f64),
    Paused,
}

#[derive(Debug, Clone)]
pub struct FlashGuard {
    cells: Vec<(f32, i8)>,
    turns: VecDeque<f64>,
    speed: f64,
    paused: bool,
}

impl Default for FlashGuard {
    fn default() -> Self {
        FlashGuard { cells: Vec::new(), turns: VecDeque::new(), speed: 1.0, paused: false }
    }
}

impl FlashGuard {
    pub fn rate(&self) -> f32 {
        self.turns.len() as f32 / 2.0 / WINDOW as f32
    }

    pub fn speed(&self) -> f64 {
        if self.paused { 0.0 } else { self.speed }
    }

    pub fn calming(&self) -> Calming {
        match () {
            _ if self.paused => Calming::Paused,
            _ if self.speed < 1.0 => Calming::Slowed(self.speed),
            _ => Calming::Steady,
        }
    }

    pub fn watch(&mut self, t: f64, luma: &[f32]) -> Calming {
        if luma.len() != CELLS {
            return self.calming();
        }
        if self.cells.len() != CELLS {
            self.cells = luma.iter().map(|l| (*l, 0)).collect();
            return self.calming();
        }
        let mut turned = 0;
        for ((anchor, way), &now) in self.cells.iter_mut().zip(luma) {
            let step = now - *anchor;
            let toward = if step > 0.0 { 1 } else { -1 };
            if step.abs() >= CHANGE && now.min(*anchor) < DARK {
                turned += usize::from(toward != *way);
                (*anchor, *way) = (now, toward);
            } else if toward == *way {
                *anchor = now;
            }
        }
        while self.turns.front().is_some_and(|&at| at <= t - WINDOW) {
            self.turns.pop_front();
        }
        if turned >= AREA {
            self.turns.push_back(t);
        }
        let rate = self.rate();
        if rate > SAFE_FLASHES && !self.paused {
            match self.speed <= SLOWEST {
                true => self.paused = true,
                false => {
                    self.speed = (self.speed * f64::from(SAFE_FLASHES / rate)).max(SLOWEST);
                    self.turns.clear();
                }
            }
        }
        self.calming()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(guard: &mut FlashGuard, hz: f64, seconds: f64, flashes: f64) -> Calming {
        let mut last = Calming::Steady;
        for n in 0..(seconds * hz) as usize {
            let t = n as f64 / hz;
            let bright = (t * flashes * guard.speed()).fract() < 0.5;
            last = guard.watch(t, &[if bright { 0.9 } else { 0.05 }; CELLS]);
        }
        last
    }

    #[test]
    fn slow_changes_and_small_flashes_pass() {
        let mut guard = FlashGuard::default();
        assert_eq!(frames(&mut guard, 60.0, 4.0, 2.0), Calming::Steady);
        let mut small = FlashGuard::default();
        for n in 0..240 {
            let mut luma = [0.05; CELLS];
            luma[..AREA - 1].fill(if n % 4 < 2 { 0.9 } else { 0.05 });
            small.watch(f64::from(n) / 60.0, &luma);
        }
        assert_eq!((small.calming(), small.rate()), (Calming::Steady, 0.0));
        let mut light = FlashGuard::default();
        for n in 0..240 {
            light.watch(f64::from(n) / 60.0, &[if n % 4 < 2 { 0.95 } else { 0.85 }; CELLS]);
        }
        assert_eq!(light.calming(), Calming::Steady);
    }

    #[test]
    fn fast_flashes_slow_the_scene_and_flashes_that_do_not_slow_down_pause_it() {
        let mut guard = FlashGuard::default();
        let Calming::Slowed(speed) = frames(&mut guard, 60.0, 8.0, 6.0) else {
            panic!("{:?}", guard.calming())
        };
        assert!((0.3..=0.5 + 1e-9).contains(&speed), "{speed}");
        let mut stubborn = FlashGuard::default();
        for n in 0..600 {
            stubborn.watch(f64::from(n) / 60.0, &[if n % 8 < 4 { 0.9 } else { 0.05 }; CELLS]);
        }
        assert_eq!((stubborn.calming(), stubborn.speed()), (Calming::Paused, 0.0));
    }
}
