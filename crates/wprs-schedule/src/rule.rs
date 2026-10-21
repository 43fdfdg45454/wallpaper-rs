use crate::solar::Location;
use chrono::{DateTime, Datelike, MappedLocalTime, NaiveTime, Offset, TimeDelta, TimeZone, Weekday};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    TimeRange { start: NaiveTime, end: NaiveTime },
    Weekdays(Vec<Weekday>),
    Daylight,
    Night,
    OnBattery(bool),
    All(Vec<Condition>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    ShowPlaylist(String),
    ShowFile(PathBuf),
    SetMaxFps(u32),
    Pause,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub when: Condition,
    pub then: Action,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Env {
    pub location: Option<Location>,
    pub on_battery: bool,
}

impl Condition {
    pub(crate) fn holds<Tz: TimeZone>(&self, at: &DateTime<Tz>, env: Env) -> bool {
        match self {
            Condition::TimeRange { start, end } => match start.cmp(end) {
                Ordering::Less => *start <= at.time() && at.time() < *end,
                Ordering::Greater => *start <= at.time() || at.time() < *end,
                Ordering::Equal => true,
            },
            Condition::Weekdays(days) => days.contains(&at.weekday()),
            Condition::Daylight => env.location.is_some_and(|l| l.is_day(at.to_utc())),
            Condition::Night => env.location.is_some_and(|l| !l.is_day(at.to_utc())),
            Condition::OnBattery(on) => *on == env.on_battery,
            Condition::All(all) => all.iter().all(|c| c.holds(at, env)),
        }
    }

    pub(crate) fn next_boundary<Tz: TimeZone>(&self, after: &DateTime<Tz>, env: Env) -> Option<DateTime<Tz>> {
        match self {
            Condition::TimeRange { start, end } => {
                [*start, *end].iter().filter_map(|t| next_local(after, *t)).min()
            }
            Condition::Weekdays(_) => next_local(after, NaiveTime::MIN),
            Condition::Daylight | Condition::Night => {
                env.location?.next_sun_change(after.to_utc()).map(|t| t.with_timezone(&after.timezone()))
            }
            Condition::OnBattery(_) => None,
            Condition::All(all) => all.iter().filter_map(|c| c.next_boundary(after, env)).min(),
        }
    }

    pub(crate) fn needs_location(&self) -> bool {
        match self {
            Condition::Daylight | Condition::Night => true,
            Condition::All(all) => all.iter().any(Condition::needs_location),
            _ => false,
        }
    }
}

impl Action {
    pub(crate) fn slot(&self) -> u8 {
        match self {
            Action::ShowPlaylist(_) | Action::ShowFile(_) => 0,
            Action::SetMaxFps(_) => 1,
            Action::Pause => 2,
        }
    }
}

fn next_local<Tz: TimeZone>(after: &DateTime<Tz>, time: NaiveTime) -> Option<DateTime<Tz>> {
    let tz = after.timezone();
    let local = |d| match tz.from_local_datetime(&(after.date_naive() + TimeDelta::days(d)).and_time(time)) {
        MappedLocalTime::Single(t) => vec![t],
        MappedLocalTime::Ambiguous(a, b) => vec![a, b],
        MappedLocalTime::None => vec![],
    };
    (0..3).flat_map(local).find(|t| t > after).into_iter().chain(next_shift(after)).min()
}

fn next_shift<Tz: TimeZone>(after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
    let tz = after.timezone();
    let offset =
        |s| DateTime::from_timestamp(s, 0).map(|t| tz.offset_from_utc_datetime(&t.naive_utc()).fix());
    let (mut lo, mut hi) = (after.timestamp(), after.timestamp() + 2 * 86_400);
    if offset(lo) == offset(hi) {
        return None;
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if offset(mid) == offset(lo) { lo = mid } else { hi = mid }
    }
    DateTime::from_timestamp(hi, 0).map(|t| t.with_timezone(&tz))
}
