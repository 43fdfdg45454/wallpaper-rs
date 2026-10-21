mod playlist;
mod rule;
mod solar;
mod state;

pub use playlist::{Order, Playlist, format_interval, parse_interval};
pub use rule::{Action, Condition, Rule};
pub use solar::{Location, Sun};
pub use state::{PlaylistState, Positions};

use chrono::{DateTime, TimeDelta, TimeZone};
use rule::Env;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    pub playlists: Vec<Playlist>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation<Tz: TimeZone> {
    pub actions: Vec<Action>,
    pub next_change: Option<DateTime<Tz>>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ScheduleError {
    #[error("playlist {0:?} is defined more than once")]
    DuplicatePlaylist(String),
    #[error("playlist {0:?} has no items")]
    EmptyPlaylist(String),
    #[error("location {0}, {1} is out of range")]
    InvalidLocation(f64, f64),
    #[error("rule {0} shows unknown playlist {1:?}")]
    UnknownPlaylist(usize, String),
    #[error("rule {0} depends on the sun but no location is set")]
    MissingLocation(usize),
}

impl Schedule {
    pub fn playlist(&self, id: &str) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.id == id)
    }

    pub fn validate(&self) -> Result<(), ScheduleError> {
        let mut ids = BTreeSet::new();
        for p in &self.playlists {
            if !ids.insert(p.id.as_str()) {
                return Err(ScheduleError::DuplicatePlaylist(p.id.clone()));
            }
            if p.items.is_empty() {
                return Err(ScheduleError::EmptyPlaylist(p.id.clone()));
            }
        }
        if let Some(l) = self.location.filter(|l| !l.is_valid()) {
            return Err(ScheduleError::InvalidLocation(l.latitude, l.longitude));
        }
        for (n, rule) in (1..).zip(&self.rules) {
            if let Action::ShowPlaylist(id) = &rule.then
                && !ids.contains(id.as_str())
            {
                return Err(ScheduleError::UnknownPlaylist(n, id.clone()));
            }
            if rule.when.needs_location() && self.location.is_none() {
                return Err(ScheduleError::MissingLocation(n));
            }
        }
        Ok(())
    }

    pub fn evaluate<Tz: TimeZone>(&self, now: &DateTime<Tz>, on_battery: bool) -> Evaluation<Tz> {
        let env = Env { location: self.location, on_battery };
        let actions = self.actions_at(now, env);
        let horizon = now.clone() + TimeDelta::days(366);
        let mut at = now.clone();
        let next_change = loop {
            match self.rules.iter().filter_map(|r| r.when.next_boundary(&at, env)).min() {
                Some(t) if t <= horizon && self.actions_at(&t, env) != actions => break Some(t),
                Some(t) if t <= horizon => at = t,
                _ => break None,
            }
        };
        Evaluation { actions, next_change }
    }

    fn actions_at<Tz: TimeZone>(&self, at: &DateTime<Tz>, env: Env) -> Vec<Action> {
        let mut actions: Vec<Action> = Vec::new();
        for rule in self.rules.iter().filter(|r| r.when.holds(at, env)) {
            if actions.iter().all(|a| a.slot() != rule.then.slot()) {
                actions.push(rule.then.clone());
            }
        }
        actions
    }
}
