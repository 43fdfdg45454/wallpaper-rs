use super::super::Content;
use super::super::output::Output;
use super::SceneState;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug)]
pub(in crate::compositor) struct Shared {
    time: f64,
    at: Option<Instant>,
}

pub(in crate::compositor) type Clock = Arc<Mutex<Shared>>;

pub(in crate::compositor) fn tick(clock: &Clock, now: Instant, running: bool, rate: f64) -> f64 {
    let mut c = clock.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(at) = c.at.filter(|at| running && now > *at) {
        c.time += now.duration_since(at).as_secs_f64() * rate.max(0.0);
    }
    if c.at.is_none_or(|at| now > at) {
        c.at = Some(now);
    }
    c.time
}

pub(in crate::compositor) fn clock_for(
    outputs: &BTreeMap<String, Output>,
    name: &str,
    content: &Content,
) -> Option<Clock> {
    let Content::Scene(spec) = content else { return None };
    spec.span?;
    let same = |s: &&SceneState| Arc::ptr_eq(&s.spec.pack, &spec.pack) && s.spec.span.is_some();
    let others = outputs.iter().filter(|(other, _)| other.as_str() != name);
    others.filter_map(|(_, o)| o.shown.scene.as_ref()).find(same).and_then(|s| s.shared.clone())
}

impl SceneState {
    pub(in crate::compositor) fn share(&mut self, clock: Option<Clock>) {
        if self.spec.span.is_none() {
            self.shared = None;
            return;
        }
        match clock {
            Some(c) if self.shared.as_ref().is_some_and(|s| Arc::ptr_eq(s, &c)) => {}
            Some(c) => {
                let time = c.lock().unwrap_or_else(|e| e.into_inner()).time;
                if (time - self.runtime.time()).abs() > 1e-3 {
                    self.resume(time);
                }
                self.shared = Some(c);
            }
            None if self.shared.is_none() => {
                self.shared = Some(Arc::new(Mutex::new(Shared { time: self.runtime.time(), at: None })));
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn screens_of_one_spread_scene_share_one_clock() {
        let clock: Clock = Arc::new(Mutex::new(Shared { time: 2.0, at: None }));
        let start = Instant::now();
        assert_eq!(tick(&clock, start, true, 1.0), 2.0);
        let later = start + Duration::from_millis(500);
        assert!((tick(&clock, later, true, 1.0) - 2.5).abs() < 1e-9);
        assert!((tick(&clock, later, true, 1.0) - 2.5).abs() < 1e-9, "a second screen at the same moment");
        assert!((tick(&clock, start, true, 1.0) - 2.5).abs() < 1e-9, "an older moment never goes back");
        let resumed = later + Duration::from_secs(60);
        assert!((tick(&clock, resumed, false, 1.0) - 2.5).abs() < 1e-9, "a pause does not count");
        let slow = resumed + Duration::from_secs(1);
        assert!((tick(&clock, slow, true, 0.5) - 3.0).abs() < 1e-9);
        assert!((tick(&clock, slow + Duration::from_secs(1), true, -4.0) - 3.0).abs() < 1e-9);
    }
}
