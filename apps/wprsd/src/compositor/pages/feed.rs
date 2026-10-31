use super::super::state::State;
use crate::inputs;
use std::time::{Duration, Instant};
use wprs_web::{Allowed, BANDS, ToWeb};

const AUDIO: Duration = Duration::from_millis(33);

impl State {
    pub(in crate::compositor) fn feed_pages(&mut self) {
        let now = Instant::now();
        for slot in self.pages.values_mut().filter(|s| s.link.is_some()) {
            let [audio, pointer, clicks] = slot.spec.pack.uses;
            let mut allowed = Allowed::default();
            let mut out = Vec::new();
            for (name, view) in slot.views.iter_mut().filter(|(_, v)| v.opened && v.live) {
                let (state, presses, [moves, presses_allowed]) = inputs::touch(name);
                let hears = audio && inputs::hears(name);
                let (moves, presses_allowed) = ((pointer || clicks) && moves, clicks && presses_allowed);
                allowed = Allowed {
                    audio: allowed.audio || hears,
                    pointer: allowed.pointer || moves,
                    clicks: allowed.clicks || presses_allowed,
                };
                let (w, h) = (f64::from(view.size.0), f64::from(view.size.1));
                let at = state.pos.filter(|_| moves).map(|[x, y]| (f64::from(x) * w, f64::from(y) * h));
                if moves && at != view.pointer {
                    view.pointer = at;
                    out.push(ToWeb::Pointer { view: view.id, at });
                }
                let pressed = presses.into_iter().filter(|_| presses_allowed).filter_map(|p| p.at);
                out.extend(pressed.map(|[x, y]| ToWeb::Press {
                    view: view.id,
                    at: (f64::from(x) * w, f64::from(y) * h),
                    button: 1,
                }));
            }
            if allowed != slot.allowed {
                slot.allowed = allowed;
                out.insert(0, ToWeb::Allowed(allowed));
            }
            if slot.audio && allowed.audio && slot.heard.is_none_or(|at| now.duration_since(at) >= AUDIO) {
                slot.heard = Some(now);
                let frame = inputs::audio();
                out.push(ToWeb::Audio {
                    left: frame.left[..BANDS].to_vec(),
                    right: frame.right[..BANDS].to_vec(),
                });
            }
            out.iter().for_each(|m| slot.send(m));
        }
    }

    pub(in crate::compositor) fn page_due(&self) -> Option<Instant> {
        let audio = self
            .pages
            .values()
            .filter(|s| s.audio && s.allowed.audio)
            .filter_map(|s| s.heard)
            .map(|at| at + AUDIO);
        let due = self.pages.values().filter_map(|s| s.due.or(s.waiting));
        let views = self.pages.values().flat_map(|s| s.views.values());
        let hide = views.filter(|v| v.live).filter_map(|v| v.unseen).map(|at| at + super::GRACE);
        let listed =
            self.pages.values().filter(|s| !s.spec.folders.is_empty() && s.views.values().any(|v| v.live));
        let scan = listed.filter_map(|s| s.scanned).map(|at| at + super::SCAN);
        audio.chain(due).chain(hide).chain(scan).min()
    }
}
