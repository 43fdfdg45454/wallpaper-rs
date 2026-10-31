use super::*;
use chrono::{FixedOffset, MappedLocalTime, NaiveDateTime};
use std::time::Duration;
use wprs_schedule::{Order, Playlist, PlaylistState};

mod changes;
mod switches;

#[derive(Clone, Copy)]
struct Shift(i64, i32, i32);

impl Shift {
    fn offset(&self, utc: i64) -> FixedOffset {
        FixedOffset::east_opt(if utc < self.0 { self.1 } else { self.2 }).unwrap()
    }
}

impl TimeZone for Shift {
    type Offset = FixedOffset;

    fn from_offset(_: &FixedOffset) -> Self {
        Shift(0, 0, 0)
    }

    fn offset_from_local_date(&self, _: &NaiveDate) -> MappedLocalTime<FixedOffset> {
        MappedLocalTime::None
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        let mut offsets =
            [self.1.max(self.2), self.1.min(self.2)].map(|s| FixedOffset::east_opt(s).unwrap()).to_vec();
        offsets.dedup();
        offsets.retain(|o| self.offset((*local - *o).and_utc().timestamp()) == *o);
        match offsets[..] {
            [one] => MappedLocalTime::Single(one),
            [first, second] => MappedLocalTime::Ambiguous(first, second),
            _ => MappedLocalTime::None,
        }
    }

    fn offset_from_utc_date(&self, _: &NaiveDate) -> FixedOffset {
        self.offset(0)
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        self.offset(utc.and_utc().timestamp())
    }
}

fn at(text: &str) -> DateTime<Utc> {
    text.parse().unwrap()
}

fn rules(json: serde_json::Value) -> Rules {
    serde_json::from_value(json).unwrap()
}

fn list(uids: &[&str], order: Order) -> Resolved {
    let items = uids.iter().map(|u| format!("/w/{u}.png").into()).collect();
    let playlist =
        Playlist { id: "P".into(), items, order, interval: Duration::from_secs(1800), per_output: true };
    Resolved { playlist, uids: uids.iter().map(|u| u.to_string()).collect() }
}

fn row() -> Rotation {
    Rotation {
        output: "DP-1".into(),
        playlist: "P".into(),
        state: PlaylistState::new(7),
        next_at: None,
        current: None,
    }
}
