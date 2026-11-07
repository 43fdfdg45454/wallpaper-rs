use super::Stage;
use crate::{extras, opened};
use std::time::Instant;
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
        if taps && down && self.touch.1 == 0 {
            self.gate.clicked(Instant::now());
        }
        if taps && buttons != self.touch.1 && self.presses.len() < MAX_PRESSES {
            self.presses.push(Press { at: Some(at), buttons });
        }
        if moves || taps {
            self.touch = (down.then_some(at).filter(|_| moves), buttons, true);
        }
        moves || taps
    }

    pub(crate) fn opening(&mut self) {
        let Some(prop) = self.acting.asked.take() else { return };
        let allowed = !self.lock && self.granted[2] && self.opens.len() < 4;
        if let Some(target) =
            opened::chosen(&self.pack, &prop).filter(|_| allowed && self.gate.lets(Instant::now()))
        {
            self.opens.push(target);
        }
    }

    pub(crate) fn opened(&mut self) -> Vec<serde_json::Value> {
        std::mem::take(&mut self.opens)
    }

    pub(crate) fn offset(&mut self, x: f32) -> bool {
        let moved = self.offset != x && self.pack.scene.tilts();
        self.offset = x;
        self.touch.2 |= moved;
        moved
    }

    pub(crate) fn hide(&mut self) {
        self.tick = None;
        self.mute();
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
