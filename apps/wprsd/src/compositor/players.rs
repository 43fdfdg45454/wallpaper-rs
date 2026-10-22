use super::output::Output;
use super::slot::PlayerSlot;
use super::state::State;
use super::{Content, OutputStatus, lock};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

fn demand(outputs: &BTreeMap<String, Output>, path: &PathBuf) -> ((u32, u32), bool) {
    let users = outputs.values().filter(|o| o.uses_video(path));
    users.fold(((0, 0), false), |((w, h), on), o| ((w.max(o.size.0), h.max(o.size.1)), on || o.active))
}

impl State {
    pub(super) fn sync_players(&mut self) {
        let (outputs, settings, tx, now) = (&self.outputs, self.settings, &self.tx, Instant::now());
        self.players.retain(|path, _| outputs.values().any(|o| o.uses_video(path)));
        for path in outputs.values().flat_map(|o| o.layers().filter_map(|s| s.content.video())) {
            if !self.players.contains_key(path) {
                let (max_size, playing) = demand(outputs, path);
                self.players.insert(path.clone(), PlayerSlot::new(max_size, playing, now));
            }
        }
        for (path, slot) in &mut self.players {
            let (max_size, active) = demand(outputs, path);
            if slot.playing != active {
                slot.player.iter().for_each(|p| p.set_playing(active));
                slot.playing = active;
            }
            if slot.max_size != max_size {
                slot.player.iter().for_each(|p| p.set_max_size(max_size));
                slot.max_size = max_size;
            }
            slot.tick(path, settings, tx, now);
        }
    }

    pub(super) fn take_video_frames(&mut self) {
        let now = Instant::now();
        for (path, slot) in &mut self.players {
            if slot.receive(path, &self.renderer, now) {
                let wants = |o: &Output| o.uses_video(path) && (o.active || o.missing_frame);
                self.outputs.values_mut().filter(|o| wants(o)).for_each(|o| o.dirty = true);
            }
        }
    }

    pub(super) fn publish_status(&self) {
        let status = |o: &Output| {
            let slot = o.shown.content.video().and_then(|p| self.players.get(p));
            let error = slot.and_then(|s| s.error.as_ref());
            let kind = match o.shown.content {
                Content::Color => "color",
                Content::Image(_) => "image",
                Content::Video(_) => "video",
            };
            let running = slot.is_some_and(|s| s.playing && s.player.is_some());
            (kind, o.active && running && error.is_none(), error)
        };
        let mut published = lock(&self.status);
        let unchanged = published.len() == self.outputs.len()
            && self.outputs.iter().all(|(name, o)| {
                let (kind, playing, error) = status(o);
                let same = |s: &OutputStatus| (s.kind, s.playing, s.error.as_ref()) == (kind, playing, error);
                published.get(name).is_some_and(same)
            });
        if !unchanged {
            let entry = |(name, o): (&String, &Output)| {
                let (kind, playing, error) = status(o);
                (name.clone(), OutputStatus { kind, playing, error: error.cloned() })
            };
            *published = self.outputs.iter().map(entry).collect();
        }
    }
}
