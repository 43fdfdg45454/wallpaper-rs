#[cfg(test)]
use super::super::scripts::Link;
use super::SceneState;
use std::time::Instant;
use wprs_render::SceneView;
use wprs_scene::Uses;

impl SceneState {
    pub(crate) fn retry(&mut self) {
        if self.view.as_mut().is_some_and(SceneView::retry) {
            self.view = None;
        }
    }

    pub(crate) fn pause(&mut self) {
        self.tick = None;
        if let Some(view) = self.view.as_mut() {
            view.trim();
            self.runtime.restart_particles();
        }
    }

    pub(crate) fn time(&self) -> f64 {
        self.runtime.time()
    }

    pub(crate) fn resume(&mut self, time: f64) {
        if !self.still && !self.hushed {
            self.runtime.seek(time);
        }
    }

    pub(crate) fn uses(&self) -> Uses {
        self.uses
    }

    pub(crate) fn playing(&self) -> bool {
        self.moving || self.host.is_some()
    }

    #[cfg(test)]
    pub(crate) fn shares(&self) -> (usize, usize) {
        self.host.as_ref().map_or((0, 0), Link::shares)
    }

    pub(crate) fn calm(&mut self, asked: bool) -> bool {
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

    pub(crate) fn guard(&mut self, on: bool) -> bool {
        let changed = on != self.guard.is_some();
        if changed {
            self.guard = on.then(|| super::super::guard::Guard::new(Instant::now()));
        }
        changed
    }

    pub(crate) fn particles(&self) -> &'static str {
        match self.view.as_ref().and_then(SceneView::gpu_particles) {
            Some(true) => "gpu",
            Some(false) => "cpu",
            None => "",
        }
    }

    pub(crate) fn flashes(&self) -> &'static str {
        self.guard.as_ref().map_or("", |g| g.label())
    }

    pub(crate) fn press(&mut self, id: &str) {
        self.runtime.press(id);
        self.pressed.push(id.to_owned());
        self.calm = 0;
    }
}
