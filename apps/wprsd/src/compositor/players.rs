use super::output::Output;
use super::power::Power;
use super::scene::SceneState;
use super::slot::{Demand, PlayerSlot};
use super::state::State;
use super::{Content, OutputStatus, Reel, lock};
use std::collections::BTreeMap;
use std::time::Instant;
use wprs_pipewire::Exporter;

fn demand(outputs: &BTreeMap<String, Output>, exporter: &Exporter, reel: &Reel, power: Power) -> Demand {
    let users = outputs.iter().filter_map(|(name, o)| Some((name, o, o.loudest(reel)?)));
    users.fold(((0, 0), false, 1, 0), |((w, h), on, fps, volume), (name, o, loud)| {
        let on = on || (o.active && o.watched(exporter.is_streaming(name)));
        ((w.max(o.size.0), h.max(o.size.1)), on, fps.max(o.fps(power)), volume.max(loud))
    })
}

impl State {
    pub(super) fn sync_players(&mut self) {
        let (outputs, exporter, power, now) = (&self.outputs, &*self.exporter, self.power, Instant::now());
        let wanted: Vec<&Reel> = outputs
            .values()
            .flat_map(|o| o.layers().flat_map(|s| s.content.videos()))
            .map(|(r, _)| r)
            .collect();
        let mut orphans: Vec<(Reel, PlayerSlot)> =
            self.players.extract_if(|reel, _| !wanted.contains(&reel)).collect();
        for reel in wanted {
            if self.players.contains_key(reel) {
                continue;
            }
            let moved = orphans.iter_mut().position(|(r, s)| {
                r.path == reel.path && s.player.as_mut().is_some_and(|p| p.set_cut(reel.cut))
            });
            let slot = match moved {
                Some(i) => orphans.swap_remove(i).1,
                None => PlayerSlot::new(demand(outputs, exporter, reel, power), now),
            };
            self.players.insert(reel.clone(), slot);
        }
        let heard = (self.settings, self.locked);
        for (reel, slot) in &mut self.players {
            let (max_size, active, fps, volume) = demand(outputs, exporter, reel, power);
            if slot.fps != fps {
                slot.player.iter_mut().for_each(|p| p.set_max_fps(fps));
                slot.fps = fps;
            }
            if slot.volume != volume {
                slot.set_volume(volume, heard, now);
            }
            if slot.playing != active {
                slot.player.iter().for_each(|p| p.set_playing(active));
                slot.playing = active;
            }
            if slot.max_size != max_size {
                slot.player.iter().for_each(|p| p.set_max_size(max_size));
                slot.max_size = max_size;
            }
            slot.tick(reel, heard, &self.tx, now);
        }
    }

    pub(super) fn take_video_frames(&mut self) {
        let now = Instant::now();
        for (reel, slot) in &mut self.players {
            if slot.receive(&reel.path, self.renderer, now) {
                let wants = |o: &Output| o.uses_video(reel) && (o.active || o.missing_frame);
                self.outputs.values_mut().filter(|o| wants(o)).for_each(|o| o.dirty = true);
            }
        }
    }

    pub(super) fn publish_status(&self) {
        let status = |name: &str, o: &Output| {
            let slot = o.shown.content.video().and_then(|p| self.players.get(p));
            let scene_error = o.shown.scene.as_ref().and_then(|s| s.error.as_ref());
            let error = slot.and_then(|s| s.error.as_ref()).or(scene_error).cloned();
            let stops = error.is_some();
            let kind = match o.shown.content {
                Content::Color => "color",
                Content::Image(_) => "image",
                Content::Video(_) => "video",
                Content::Scene(_) => "scene",
            };
            let scene = o.shown.scene.as_ref().is_some_and(SceneState::playing);
            let running = scene || slot.is_some_and(|s| s.playing && s.player.is_some());
            let watched = o.active && o.watched(self.exporter.is_streaming(name));
            let playing = watched && running && !stops;
            let flashes = o.shown.scene.as_ref().map_or("", SceneState::flashes);
            let particles = o.shown.scene.as_ref().map_or("", SceneState::particles);
            let quality = o.shown.scene.as_ref().map_or(("", ""), SceneState::quality);
            (
                kind,
                playing,
                error,
                self.moves(o),
                o.fps(self.power),
                self.sound(name, o),
                (flashes, particles, quality),
            )
        };
        let mut published = lock(&self.status);
        let unchanged = published.len() == self.outputs.len()
            && self.outputs.iter().all(|(name, o)| {
                let (kind, playing, error, parallax, fps, sound, flashes) = status(name, o);
                let left = left(o);
                let lines = || console(name, o);
                let same = |s: &OutputStatus| {
                    let now = (kind, playing, &error, parallax, fps, sound, flashes, left);
                    (
                        s.kind,
                        s.playing,
                        &s.error,
                        s.parallax,
                        s.fps,
                        s.sound,
                        (s.flashes, s.particles, (s.quality, s.quality_reason)),
                        &s.left_out[..],
                    ) == now
                };
                published.get(name).is_some_and(|s| same(s) && s.console.iter().eq(lines()))
            });
        if !unchanged {
            let entry = |(name, o): (&String, &Output)| {
                let (kind, playing, error, parallax, fps, sound, (flashes, particles, quality)) =
                    status(name, o);
                let (quality, quality_reason) = quality;
                let frames = published.get(name).map_or(0, |s| s.frames);
                let (left_out, console) = (left(o).to_vec(), console(name, o).cloned().collect());
                let status = OutputStatus {
                    kind,
                    playing,
                    error,
                    parallax,
                    frames,
                    fps,
                    sound,
                    flashes,
                    particles,
                    quality,
                    quality_reason,
                    left_out,
                    console,
                };
                (name.clone(), status)
            };
            let fresh = self.outputs.iter().map(entry).collect();
            let old = std::mem::replace(&mut *published, fresh);
            let names = published.keys().chain(old.keys());
            let mut changed: Vec<String> =
                names.filter(|n| published.get(*n) != old.get(*n)).cloned().collect();
            changed.sort();
            changed.dedup();
            drop(published);
            (self.on_status)(changed);
        }
    }
}

fn left(o: &Output) -> &[String] {
    o.shown.scene.as_ref().map_or(&[], |s| &s.left)
}

fn console<'a>(name: &str, o: &'a Output) -> impl Iterator<Item = &'a String> {
    let shown = o.shown.scene.as_ref().filter(|_| name.starts_with("preview:"));
    shown.into_iter().flat_map(|s| &s.console)
}
