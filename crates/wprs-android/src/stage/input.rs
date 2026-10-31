use super::Stage;
use crate::extras;
use wprs_render::SceneView;
use wprs_scene::{MAX_PRESSES, Press};

impl Stage {
    pub(crate) fn turn(&mut self) {
        if self.extras.next.is_some_and(|at| extras::now() >= at) {
            self.gather();
        }
    }

    pub(crate) fn until_turn(&self, delay: i64) -> i64 {
        let Some(at) = self.extras.next else { return delay };
        let left = (at.saturating_sub(extras::now()) * 1000).max(1) as i64;
        if delay == 0 { left } else { delay.min(left) }
    }

    pub(crate) fn touch(&mut self, at: [f32; 2], down: bool) -> bool {
        let (moves, taps) = self.allowed();
        let buttons = u8::from(down);
        if taps && buttons != self.touch.1 && self.presses.len() < MAX_PRESSES {
            self.presses.push(Press { at: Some(at), buttons });
        }
        if moves || taps {
            self.touch = (down.then_some(at).filter(|_| moves), buttons, true);
        }
        moves || taps
    }

    pub(crate) fn offset(&mut self, x: f32) -> bool {
        let moved = self.offset != x && self.pack.scene.tilts();
        self.offset = x;
        self.touch.2 |= moved;
        moved
    }

    pub(crate) fn hide(&mut self) {
        self.tick = None;
        if let Some(view) = self.view.as_mut() {
            view.trim();
            self.runtime.restart_particles();
        }
    }

    pub(crate) fn drop_view(&mut self) {
        self.view = None;
    }

    pub(crate) fn rest(&mut self) {
        if self.due.is_none() {
            self.view.iter_mut().for_each(SceneView::trim);
        }
    }
}
