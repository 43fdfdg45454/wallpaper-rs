use super::SceneInput;
use super::scene::SceneState;
use super::state::State;
use crate::inputs;
use std::time::Instant;
use wprs_scene::{AudioFrame, Uses};

pub(super) fn declare(output: &str, scene: Option<&SceneState>) {
    let pack = scene.map(|s| &s.spec.pack);
    let named = pack.map(|p| (p.props.id.as_str(), p.props.title.as_str(), p.key.as_str()));
    let uses = scene.map(SceneState::uses).unwrap_or_default();
    inputs::needs(output, named.unwrap_or_default(), uses.audio, uses.cursor);
}

struct Live {
    name: String,
    uses: Uses,
    due: bool,
    waiting: bool,
}

impl State {
    fn feed(&mut self, input: SceneInput) {
        let (SceneInput::Audio(name, _) | SceneInput::Pointer(name, _) | SceneInput::Script(name, _)) =
            &input;
        if let Some(o) = self.outputs.get_mut(name) {
            let changed = o.shown.scene.as_mut().is_some_and(|scene| scene.input(input));
            o.dirty |= changed && o.active;
        }
    }

    pub(super) fn feed_inputs(&mut self) {
        let now = Instant::now();
        let live: Vec<Live> = (self.outputs.iter())
            .filter(|(name, o)| o.active && o.open(self.exporter.is_streaming(name)))
            .filter_map(|(name, o)| {
                let s = o.shown.scene.as_ref()?;
                let due = o.dirty || s.due.is_none_or(|at| at <= now);
                Some(Live { name: name.clone(), uses: s.uses(), due, waiting: due || !s.moving })
            })
            .collect();
        let hearing: Vec<bool> =
            live.iter().map(|l| l.uses.audio && l.waiting && inputs::hears(&l.name)).collect();
        let heard = hearing.contains(&true).then(inputs::audio).unwrap_or_default();
        for (live, hears) in live.into_iter().zip(hearing) {
            if live.uses.audio && live.waiting {
                let audio = if hears { heard.clone() } else { AudioFrame::default() };
                self.feed(SceneInput::Audio(live.name.clone(), Box::new(audio)));
            }
            if live.uses.cursor {
                let pointer = inputs::pointer(&live.name);
                self.feed(SceneInput::Pointer(live.name.clone(), pointer));
            }
            let scene = self.outputs.get_mut(&live.name).and_then(|o| o.shown.scene.as_mut());
            if let Some(outputs) = scene.filter(|_| live.due).and_then(SceneState::script) {
                self.feed(SceneInput::Script(live.name, outputs));
            }
        }
    }
}
