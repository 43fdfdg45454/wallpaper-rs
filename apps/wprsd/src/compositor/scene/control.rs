#[cfg(test)]
use super::super::scripts::Link;
use super::SceneState;
use std::time::Instant;
use wprs_render::SceneView;
use wprs_scene::Uses;

impl SceneState {
    pub(in crate::compositor) fn retry(&mut self) {
        if self.view.as_mut().is_some_and(SceneView::retry) {
            self.view = None;
        }
    }

    pub(in crate::compositor) fn pause(&mut self) {
        self.tick = None;
        if let Some(view) = self.view.as_mut() {
            view.trim();
            self.runtime.restart_particles();
        }
    }

    pub(in crate::compositor) fn time(&self) -> f64 {
        self.runtime.time()
    }

    pub(in crate::compositor) fn resume(&mut self, time: f64) {
        if !self.still && !self.hushed {
            self.runtime.seek(time);
        }
    }

    pub(in crate::compositor) fn tape(&self, asset: &str) -> Option<&wprs_scene::Tape> {
        self.runtime.tapes().iter().find(|t| t.asset == asset)
    }

    pub(in crate::compositor) fn uses(&self) -> Uses {
        self.uses
    }

    pub(in crate::compositor) fn playing(&self) -> bool {
        self.moving || self.host.is_some()
    }

    #[cfg(test)]
    pub(in crate::compositor) fn shares(&self) -> (usize, usize) {
        self.host.as_ref().map_or((0, 0), Link::shares)
    }

    pub(in crate::compositor) fn calm(&mut self, asked: bool) -> bool {
        self.quiet = asked;
        let calm = asked && !self.spec.moving;
        let parallax = self.runtime.set_reduced_motion(calm);
        if std::mem::replace(&mut self.hushed, calm) == calm {
            return parallax;
        }
        if calm && !self.still {
            self.runtime.seek(f64::from(self.runtime.scene.clock.poster_time));
        }
        true
    }

    pub(in crate::compositor) fn guard(&mut self, on: bool) -> bool {
        let changed = on != self.guard.is_some();
        if changed {
            self.guard = on.then(|| super::super::guard::Guard::new(Instant::now()));
        }
        changed
    }

    pub(in crate::compositor) fn particles(&self) -> &'static str {
        match self.view.as_ref().and_then(SceneView::gpu_particles) {
            Some(true) => "gpu",
            Some(false) => "cpu",
            None => "",
        }
    }

    pub(in crate::compositor) fn flashes(&self) -> &'static str {
        self.guard.as_ref().map_or("", |g| g.label())
    }

    pub(in crate::compositor) fn press(&mut self, id: &str) {
        self.runtime.press(id);
        self.pressed.push(id.to_owned());
        self.calm = 0;
    }
}
