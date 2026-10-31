use super::{MAX_STEP, Runtime};
use crate::Values;
use crate::input::{AudioFrame, PointerState, ScriptOutputs, TimelineCommand};
use crate::uses;
use std::collections::BTreeSet;

impl Runtime {
    pub fn set_language(&mut self, lang: &str) {
        self.lang = lang.to_owned();
    }

    pub fn set_props(&mut self, props: Values) -> bool {
        let changed: Vec<String> =
            props.iter().filter(|(k, v)| self.props.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect();
        self.props = props;
        if !changed.is_empty() {
            self.uses = uses::of(&self.scene, &self.props);
        }
        self.trigger(|t| changed.iter().any(|id| t.strip_prefix("prop:") == Some(id)));
        !changed.is_empty()
    }

    pub fn set_audio(&mut self, audio: AudioFrame) -> bool {
        let dt = (self.t - std::mem::replace(&mut self.heard, self.t)).clamp(0.0, 1.0) as f32;
        let audio = match self.scene.audio {
            Some(h) => h.hear(&self.audio, audio, dt),
            None => audio,
        };
        let beat = audio.beat && !self.audio.beat;
        let changed = self.uses().audio && audio != self.audio && !(audio.silent && self.audio.silent);
        if !audio.silent {
            self.stirred[1] = self.t;
        }
        self.audio = audio;
        if beat {
            self.beat = self.t;
            self.trigger(|t| t == "beat");
        }
        changed
    }

    pub fn set_pointer(&mut self, pointer: PointerState) -> bool {
        let changed = self.uses().cursor && pointer != self.pointer;
        if pointer.moving() || pointer.buttons & !self.pointer.buttons != 0 {
            self.stirred[0] = self.t;
        }
        let delay = self.scene.canvas.parallax.as_ref().map_or(crate::input::SETTLE, |g| g.delay);
        let dt = (self.t - self.glided).clamp(0.0, MAX_STEP) as f32;
        self.glide = match delay == crate::input::SETTLE {
            true => pointer,
            false => self.glide.follow_in(pointer.pos, pointer.buttons, dt, delay),
        };
        (self.pointer, self.glided) = (pointer, self.t);
        changed
    }

    pub fn set_tilt(&mut self, tilt: [f32; 2]) -> bool {
        let tilt = tilt.map(|v| v.clamp(-1.0, 1.0));
        let changed =
            !self.still && self.scene.tilts() && (0..2).any(|i| (tilt[i] - self.tilt[i]).abs() > 1e-4);
        self.tilt = tilt;
        changed
    }

    pub fn set_reduced_motion(&mut self, still: bool) -> bool {
        let moves = self.scene.layers().any(|l| !l.parallax.is_zero());
        moves && std::mem::replace(&mut self.still, still) != still
    }

    pub fn set_sheet(&mut self, asset: &str, durations: Vec<f32>) {
        self.sheets.insert(asset.to_owned(), durations);
    }

    pub fn set_script(&mut self, outputs: ScriptOutputs) -> bool {
        let outputs = outputs.bounded(&self.scene);
        self.play_sounds(&outputs.sounds);
        for (id, command) in &outputs.timelines {
            let Some((tl, head)) = self.timelines.iter_mut().find(|(tl, _)| &tl.id == id) else { continue };
            match *command {
                TimelineCommand::Play => (head.playing, head.reversed) = (true, false),
                TimelineCommand::Pause => head.playing = false,
                TimelineCommand::Seek(t) => head.t = t.clamp(0.0, tl.length()),
                TimelineCommand::Reverse => (head.playing, head.reversed) = (true, !head.reversed),
                TimelineCommand::Stop => (head.t, head.playing, head.reversed) = (0.0, false, false),
                TimelineCommand::Speed(rate) => head.rate = rate.clamp(-100.0, 100.0),
            }
        }
        for (layer, control) in &outputs.particles {
            let system = self.scene.layer(layer).and_then(|l| match &l.kind {
                crate::Kind::Particles { system, .. } => {
                    self.scene.particles.keys().position(|k| k == system)
                }
                _ => None,
            });
            let at = system.map(|s| crate::particles::flock_of(&self.flocks, s, layer));
            if let Some(swarm) = at.and_then(|i| self.swarms.get_mut().get_mut(i)) {
                control.commands.iter().for_each(|c| swarm.command(*c));
            }
        }
        self.adopt(outputs)
    }

    pub fn set_playing(&mut self, assets: BTreeSet<String>) {
        self.playing = assets;
    }

    pub fn press(&mut self, id: &str) {
        self.trigger(|t| t.strip_prefix("prop:") == Some(id));
    }
}
