use super::output::Output;
use super::slot::PlayerSlot;
use super::{Content, Reel};
use std::collections::BTreeMap;
use wprs_video::Cut;

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Steer {
    cut: Cut,
    jumps: u32,
    at: f64,
    pub(super) wants: bool,
}

pub(super) fn steer(outputs: &BTreeMap<String, Output>, reel: &Reel) -> Option<Steer> {
    outputs.values().flat_map(Output::layers).find_map(|s| {
        let (Content::Scene(spec), Some(scene)) = (&s.content, &s.scene) else { return None };
        let (id, ..) = spec.reels().find(|(_, r, _)| *r == reel)?;
        let t = scene.tape(id)?;
        let ms = |s: f64| (s * 1000.0).clamp(0.0, f64::from(u32::MAX)) as u32;
        let speed = ((t.rate * 100.0).round() as u32).clamp(10, 400);
        let cut = Cut { speed, start: ms(t.start), end: t.end.map_or(0, ms) };
        let wants = t.shown && t.playing && !t.ended && t.rate > 0.0;
        Some(Steer { cut, jumps: t.jumps, at: t.at, wants })
    })
}

impl PlayerSlot {
    pub(super) fn steer(&mut self, steer: Option<Steer>) {
        let Some(s) = steer else { return };
        let Some(player) = self.player.as_mut() else { return self.steered = None };
        let last = self.steered.replace((s.cut, s.jumps));
        if last.is_none_or(|(cut, _)| cut != s.cut) {
            player.set_cut(s.cut);
        }
        if last.map_or(s.at > 0.05, |(_, jumps)| jumps != s.jumps) {
            player.seek(s.at);
        }
    }
}
