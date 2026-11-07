use super::Stage;
use crate::power::Power;
use crate::settings;
use crate::sound::{self, Gate, Want};
use std::time::Instant;

impl Stage {
    pub(super) fn listen(&mut self, power: Power, still: bool) -> Option<i64> {
        if self.pack.sounds.is_empty() {
            return None;
        }
        let gate = Gate { silent: self.silent, shown: self.audible, running: power.moving() && !still };
        let decided = sound::decide(&settings::playback(), sound::property(&self.runtime), gate);
        let wait = self.sounds.follow(&self.runtime, decided, Instant::now());
        wait.map(|wait| i64::try_from(wait.as_millis()).unwrap_or(i64::MAX).max(1))
    }

    pub(crate) fn mute(&mut self) {
        self.audible = false;
        self.sounds.follow(&self.runtime, (Want::Held, 0.0), Instant::now());
    }

    pub(crate) fn sounding(&self) -> bool {
        self.sounds.sounding()
    }
}

pub(super) fn soonest(delay: i64, sound: Option<i64>) -> i64 {
    match (delay, sound) {
        (0, Some(sound)) => sound,
        (delay, Some(sound)) => delay.min(sound),
        (delay, None) => delay,
    }
}
