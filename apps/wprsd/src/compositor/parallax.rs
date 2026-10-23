use super::output::Output;
use super::state::State;
use wprs_core::edit::Motion as Source;

pub(super) const SOFTWARE: (u32, u32) = (8, 15);

impl State {
    pub(super) fn relief_limits(&self) -> Option<u32> {
        match (self.motion.software, self.settings.software_parallax) {
            (false, _) => Some(u32::MAX),
            (true, true) => Some(SOFTWARE.0),
            (true, false) => None,
        }
    }

    pub(super) fn moves(&self, output: &Output) -> bool {
        output.shown.relief.is_some() && !self.motion.calm && self.relief_limits().is_some()
    }

    pub(super) fn follow(&mut self, name: &str) {
        let moving = self.outputs.get(name).is_some_and(|o| self.moves(o));
        let source = self.outputs.get_mut(name).and_then(|o| {
            o.moving = moving;
            o.shown.relief.as_ref().map(|(_, p)| p.source)
        });
        crate::inputs::relief(name, moving && source == Some(Source::Pointer));
    }

    pub(super) fn calm(&mut self, calm: bool) {
        if std::mem::replace(&mut self.motion.calm, calm) != calm {
            self.restill();
        }
    }

    pub(super) fn restill(&mut self) {
        let names: Vec<String> = self.outputs.keys().cloned().collect();
        names.iter().for_each(|name| self.follow(name));
        for output in self.outputs.values_mut().filter(|o| o.shown.relief.is_some()) {
            (output.offset, output.dirty, output.stale) = ([0.0; 2], true, true);
        }
    }
}
