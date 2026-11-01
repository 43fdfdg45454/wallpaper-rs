use super::super::power::Power;
use super::super::{SceneInput, SceneSpec};
use super::SceneState;
use wprs_render::{GpuImage, SceneView};
use wprs_scene::MAX_PRESSES;

impl SceneState {
    pub(in crate::compositor) fn update(&mut self, spec: SceneSpec) -> bool {
        let mut changed = self.runtime.set_props(spec.values.clone());
        let media = self.spec.media() != spec.media();
        if media {
            self.runtime.set_playing(spec.clips.iter().map(|(asset, _)| asset.clone()).collect());
        }
        self.restock |= changed || media;
        self.exposed = wprs_scene::exposed(&spec.pack.props.properties, self.runtime.props());
        if spec.hold != self.spec.hold {
            spec.hold.into_iter().for_each(|time| self.runtime.seek(time));
            (self.still, self.moving, changed) = (spec.hold.is_some(), spec.hold.is_none(), true);
        }
        let moved = spec.moving != self.spec.moving;
        (self.spec, self.uses) = (spec, self.needs(self.quality));
        changed |= moved && self.calm(self.quiet);
        if changed {
            self.calm = 0;
        }
        changed || media
    }

    pub(in crate::compositor) fn input(&mut self, input: SceneInput) -> bool {
        if self.guard.as_ref().is_some_and(|g| g.speed() == 0.0) {
            return false;
        }
        match input {
            SceneInput::Audio(_, audio) => {
                let last = self.runtime.audio();
                let heard = self.uses.audio && *audio != *last && !(audio.silent && last.silent);
                self.runtime.set_audio(*audio);
                heard
            }
            SceneInput::Pointer(_, pointer, presses, allowed) => {
                let moved = std::mem::replace(&mut self.pointer, pointer) != pointer;
                let room = MAX_PRESSES.saturating_sub(self.presses.len());
                let pressed = !presses.is_empty();
                self.presses.extend(presses.into_iter().take(room));
                self.allowed = allowed;
                (moved || pressed) && self.uses.cursor
            }
            SceneInput::Script(_, outputs) => {
                let changed = self.runtime.set_script(*outputs);
                self.calm = if changed { 0 } else { self.calm };
                changed
            }
        }
    }

    pub(in crate::compositor) fn clean(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.stats().damage == 0)
    }

    pub(in crate::compositor) fn image(&self) -> Option<&GpuImage> {
        self.view.as_ref().and_then(SceneView::image)
    }

    pub(in crate::compositor) fn fps(&self, power: Power, refresh: u32) -> u32 {
        power.fps(&self.runtime, self.quality, refresh)
    }
}
