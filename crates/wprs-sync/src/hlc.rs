use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hlc(pub u64, pub u32, pub String);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Clock {
    pub ms: u64,
    pub n: u32,
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

impl Clock {
    pub fn tick(&mut self, now: u64, device: &str) -> Hlc {
        (self.ms, self.n) = if now > self.ms { (now, 0) } else { (self.ms, self.n + 1) };
        Hlc(self.ms, self.n, device.to_owned())
    }

    pub fn observe(&mut self, seen: &Hlc) {
        if seen.0 < u64::MAX && (seen.0, seen.1) > (self.ms, self.n) {
            (self.ms, self.n) = (seen.0, seen.1);
        }
    }
}
