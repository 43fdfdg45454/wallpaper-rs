use std::time::{Duration, Instant};

const STARVED_AFTER: u32 = 3;

#[derive(Debug, Default)]
pub struct Pacer {
    pending: Option<Instant>,
    starved: bool,
}

impl Pacer {
    pub fn ready(&self) -> bool {
        self.pending.is_none()
    }

    pub fn requested(&mut self, now: Instant) {
        self.pending = Some(now);
    }

    pub fn deadline(&self, period: Duration) -> Option<Instant> {
        self.pending.filter(|_| !self.starved).map(|at| at + period * STARVED_AFTER)
    }

    pub fn starve(&mut self, period: Duration, now: Instant) -> bool {
        let due = self.deadline(period).is_some_and(|at| at <= now);
        self.starved |= due;
        due
    }

    pub fn done(&mut self) -> bool {
        self.pending = None;
        std::mem::take(&mut self.starved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_wait_for_the_frame_callback() {
        let (mut pacer, now) = (Pacer::default(), Instant::now());
        assert!(pacer.ready() && pacer.deadline(Duration::from_millis(16)).is_none());
        pacer.requested(now);
        assert!(!pacer.ready());
        assert!(!pacer.done() && pacer.ready());
    }

    #[test]
    fn three_missed_refreshes_starve_once_and_the_next_callback_resumes() {
        let (mut pacer, now, period) = (Pacer::default(), Instant::now(), Duration::from_millis(10));
        pacer.requested(now);
        assert_eq!(pacer.deadline(period), Some(now + period * 3));
        assert!(!pacer.starve(period, now + period * 2));
        assert!(pacer.starve(period, now + period * 3));
        assert!(!pacer.starve(period, now + period * 9) && pacer.deadline(period).is_none());
        assert!(!pacer.ready());
        assert!(pacer.done() && pacer.ready());
        pacer.requested(now + period * 10);
        assert!(!pacer.starve(period, now + period * 11) && !pacer.done());
    }
}
