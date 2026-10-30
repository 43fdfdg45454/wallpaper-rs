use crate::{Kind, MAX_SOUNDS, Runtime, SoundCommand, SoundControl};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Taken {
    pub(crate) since: Option<f64>,
    offset: f64,
}

#[derive(Debug, Default)]
pub(crate) struct Played {
    taken: Vec<Option<Taken>>,
    volumes: Vec<f32>,
    pub(crate) heard: RefCell<Vec<bool>>,
}

impl Played {
    pub(crate) fn taken(&self, i: usize) -> Option<Taken> {
        self.taken.get(i).copied().flatten()
    }

    pub(crate) fn volume(&self, i: usize) -> f32 {
        self.volumes.get(i).copied().unwrap_or(1.0)
    }
}

impl Runtime {
    fn sound_ids(&self) -> Vec<&str> {
        let sounds = self.scene.layers().filter(|l| matches!(l.kind, Kind::Sound(_)));
        sounds.map(|l| l.id.as_str()).take(MAX_SOUNDS).collect()
    }

    pub(crate) fn play_sounds(&mut self, controls: &BTreeMap<String, SoundControl>) {
        let (ids, t) = (self.sound_ids(), self.time());
        let found: Vec<(usize, &SoundControl)> =
            controls.iter().filter_map(|(id, c)| Some((ids.iter().position(|s| s == id)?, c))).collect();
        let n = ids.len();
        self.played.taken.resize(n, None);
        self.played.volumes.resize(n, 1.0);
        let heard = self.played.heard.borrow().clone();
        for (i, control) in found {
            if let Some(v) = control.volume.filter(|v| v.is_finite()) {
                self.played.volumes[i] = v.clamp(0.0, 1.0);
            }
            if control.commands.is_empty() {
                continue;
            }
            let sound = self.scene.sounds().nth(i).filter(|s| s.play_on.is_some());
            let onset = if sound.is_some() { self.onsets[i] } else { self.gates.borrow()[i] };
            let since = onset.filter(|_| heard.get(i) == Some(&true));
            let taken = self.played.taken[i].get_or_insert(Taken { since, offset: 0.0 });
            for command in &control.commands {
                match command {
                    SoundCommand::Play => taken.since = taken.since.or(Some(t - taken.offset)),
                    SoundCommand::Pause => {
                        if let Some(s) = taken.since.take() {
                            taken.offset = t - s;
                        }
                    }
                    SoundCommand::Stop => *taken = Taken { since: None, offset: 0.0 },
                }
            }
        }
    }

    pub fn sounds_playing(&self) -> Vec<String> {
        let heard = self.played.heard.borrow();
        let ids = self.sound_ids().into_iter().zip(heard.iter());
        ids.filter(|(_, on)| **on).map(|(id, _)| id.to_owned()).collect()
    }
}
