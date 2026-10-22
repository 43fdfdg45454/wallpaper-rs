use chrono::{Datelike, FixedOffset, Local, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime};
use chrono::{Offset, TimeZone, Timelike, Utc};
use jiff::tz::AmbiguousOffset;
use std::sync::{PoisonError, RwLock};

static NAMED: RwLock<Option<jiff::tz::TimeZone>> = RwLock::new(None);

#[derive(Debug, Clone, Copy)]
pub struct Zone;

pub fn follow(name: &str) {
    match jiff::tz::TimeZone::get(name) {
        Ok(zone) => {
            *NAMED.write().unwrap_or_else(PoisonError::into_inner) = Some(zone);
            tracing::debug!(zone = name, "time zone changed");
        }
        Err(e) => tracing::warn!("cannot use the time zone {name:?}: {e}"),
    }
}

fn named() -> Option<jiff::tz::TimeZone> {
    NAMED.read().unwrap_or_else(PoisonError::into_inner).clone()
}

fn fixed(offset: jiff::tz::Offset) -> FixedOffset {
    FixedOffset::east_opt(offset.seconds()).unwrap_or_else(|| Utc.fix())
}

fn at_local(zone: &jiff::tz::TimeZone, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
    let small = |v: u32| v as i8;
    let (date, time) = (local.date(), local.time());
    let civil = i16::try_from(date.year()).ok().and_then(|year| {
        let nanos = time.nanosecond().min(999_999_999) as i32;
        let (hour, minute, second) = (small(time.hour()), small(time.minute()), small(time.second()));
        jiff::civil::DateTime::new(year, small(date.month()), small(date.day()), hour, minute, second, nanos)
            .ok()
    });
    match civil.map(|c| zone.to_ambiguous_timestamp(c).offset()) {
        Some(AmbiguousOffset::Unambiguous { offset }) => MappedLocalTime::Single(fixed(offset)),
        Some(AmbiguousOffset::Fold { before, after }) => {
            MappedLocalTime::Ambiguous(fixed(before), fixed(after))
        }
        Some(AmbiguousOffset::Gap { .. }) | None => MappedLocalTime::None,
    }
}

fn at_utc(zone: &jiff::tz::TimeZone, utc: &NaiveDateTime) -> FixedOffset {
    jiff::Timestamp::from_second(utc.and_utc().timestamp())
        .map_or_else(|_| Utc.fix(), |t| fixed(zone.to_offset(t)))
}

impl TimeZone for Zone {
    type Offset = FixedOffset;

    fn from_offset(_: &FixedOffset) -> Self {
        Zone
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<FixedOffset> {
        self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN))
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        named().map_or_else(|| Local.offset_from_local_datetime(local), |zone| at_local(&zone, local))
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        self.offset_from_utc_datetime(&utc.and_time(NaiveTime::MIN))
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        named().map_or_else(|| Local.offset_from_utc_datetime(utc), |zone| at_utc(&zone, utc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daylight_saving_zone_gives_offsets_gaps_and_folds() {
        let zone = jiff::tz::TimeZone::posix("EST5EDT,M3.2.0,M11.1.0").unwrap();
        let at = |d: u32, m: u32, h: u32, min: u32| {
            NaiveDate::from_ymd_opt(2026, m, d).unwrap().and_hms_opt(h, min, 0).unwrap()
        };
        let hours = |h: i32| FixedOffset::east_opt(h * 3600).unwrap();
        assert_eq!(
            (at_utc(&zone, &at(15, 1, 12, 0)), at_utc(&zone, &at(15, 7, 12, 0))),
            (hours(-5), hours(-4))
        );
        assert_eq!(at_local(&zone, &at(8, 3, 2, 30)), MappedLocalTime::None);
        assert_eq!(at_local(&zone, &at(1, 11, 1, 30)), MappedLocalTime::Ambiguous(hours(-4), hours(-5)));
        assert_eq!(at_local(&zone, &at(1, 6, 9, 0)), MappedLocalTime::Single(hours(-4)));
    }
}
