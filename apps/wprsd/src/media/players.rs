use super::track::{Track, dict, number, word};
use std::collections::HashMap;
use std::time::Instant;
use zbus::OwnedVariant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) enum Status {
    #[default]
    Stopped,
    Paused,
    Playing,
}

#[derive(Debug, Clone)]
pub(crate) struct Player {
    pub(crate) status: Status,
    pub(crate) track: Track,
    pub(crate) position: f64,
    pub(crate) read: Instant,
    pub(crate) rate: f64,
    pub(crate) since: Instant,
}

impl Player {
    pub(crate) fn new(now: Instant) -> Player {
        let (status, track) = Default::default();
        Player { status, track, position: 0.0, read: now, rate: 1.0, since: now }
    }

    pub(crate) fn apply(&mut self, props: &HashMap<String, OwnedVariant>, now: Instant) -> bool {
        let mut timed = false;
        if let Some(status) = props.get("PlaybackStatus").and_then(|v| word(v)) {
            let status = match status.as_str() {
                "Playing" => Status::Playing,
                "Paused" => Status::Paused,
                _ => Status::Stopped,
            };
            if status != self.status {
                self.position = self.now(now);
                (self.status, self.since, self.read, timed) = (status, now, now, true);
            }
        }
        if let Some(rate) = props.get("Rate").and_then(|v| number(v)) {
            self.position = self.now(now);
            (self.rate, self.read, timed) = (rate.clamp(-100.0, 100.0), now, true);
        }
        if let Some(position) = props.get("Position").and_then(|v| number(v)) {
            self.seek(position, now);
        }
        if let Some(data) = props.get("Metadata") {
            let track = Track::read(&dict(data));
            if !track.same_song(&self.track) {
                (self.position, self.read, timed) = (0.0, now, true);
            }
            self.track = track;
        }
        timed
    }

    pub(crate) fn seek(&mut self, micros: f64, now: Instant) {
        (self.position, self.read) = ((micros / 1e6).max(0.0), now);
    }

    pub(crate) fn now(&self, now: Instant) -> f64 {
        let spent = now.saturating_duration_since(self.read).as_secs_f64();
        let moved = match self.status {
            Status::Playing => self.position + spent * self.rate,
            _ => self.position,
        };
        match self.track.length {
            length if length > 0.0 => moved.clamp(0.0, length),
            _ => moved.max(0.0),
        }
    }
}

pub(crate) fn chosen(players: &HashMap<String, Player>) -> Option<(&String, &Player)> {
    players.iter().max_by_key(|(name, p)| (p.status, p.track.known(), p.since, std::cmp::Reverse(*name)))
}
