use super::{IDLE, LISTEN, RETRY, STEP, Session};
use std::time::Duration;
use wprs_core::rgb::Covered;
use wprs_rgb::Strip;

impl Session {
    pub(super) fn held(&self) -> bool {
        let covered = |(source, _): &(String, Strip)| super::super::covered::covered(source);
        self.rgb.when_covered == Covered::Pause && self.last.as_ref().is_some_and(covered)
    }

    pub(super) fn wait(&self) -> Duration {
        match (&self.test, &self.client) {
            (Some(_), _) => STEP,
            (None, Some(_)) if !self.held() => LISTEN,
            (None, None) if self.retries() => RETRY,
            _ => IDLE,
        }
    }

    pub(super) fn retries(&self) -> bool {
        self.rgb.enabled && self.rgb.when_covered == Covered::Follow && super::super::covered::any()
    }

    pub(super) fn retry(&mut self) {
        if self.client.is_none() && self.retries() {
            self.ready().ok();
        }
    }
}
