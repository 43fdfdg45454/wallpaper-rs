use super::{MAX_GAP, MAX_STEP, Runtime};
use crate::particles::Swarm;
use std::cell::RefCell;

impl Runtime {
    pub fn seek(&mut self, t: f64) {
        self.timelines.iter_mut().for_each(|(tl, head)| *head = tl.start());
        (self.tapes, self.ended) = (crate::footage::tapes(&self.scene), Vec::new());
        (self.t, self.stirred, self.beat) = (0.0, [f64::NEG_INFINITY; 2], f64::NEG_INFINITY);
        self.swarms.get_mut().iter_mut().for_each(Swarm::restart);
        self.puppets = super::puppets::cast(&self.scene);
        self.step_puppets(0.0);
        self.pose_puppets();
        (self.onsets, self.gates) =
            (crate::voice::onsets(&self.scene), RefCell::new(vec![None; self.onsets.len()]));
        while self.t < t {
            self.advance((t - self.t).min(MAX_STEP));
        }
        self.fired.clear();
        self.played = Default::default();
    }

    pub fn restart_particles(&mut self) {
        self.swarms.get_mut().iter_mut().for_each(Swarm::restart);
    }

    pub fn advance(&mut self, dt: f64) {
        let mut left = dt.clamp(0.0, MAX_GAP);
        loop {
            let step = left.min(MAX_STEP);
            (self.t, self.dt, left) = (self.t + step, step as f32, left - step);
            self.tick(step as f32);
            self.roll(step);
            self.step_puppets(step as f32);
            if left <= 0.0 {
                break;
            }
        }
        self.pose_puppets();
    }

    pub fn set_wall(&mut self, local: f64) {
        self.wall = local;
    }

    pub fn wall(&self) -> f64 {
        self.wall
    }

    pub fn parallax(&self) -> [f32; 2] {
        let glide = crate::kin::glide(&self.scene, &self.script);
        let tilt = if glide.gyro && !self.still { self.tilt } else { [0.0; 2] };
        [0, 1].map(|i| ((self.glide.smooth[i] - 0.5) * glide.pointer + tilt[i] * 0.5) * glide.amount)
    }
}
