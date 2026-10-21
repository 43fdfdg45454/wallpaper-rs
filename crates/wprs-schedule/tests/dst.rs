use chrono::{DateTime, FixedOffset, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use wprs_schedule::{Action, Schedule};

#[derive(Debug, Clone, Copy, PartialEq)]
struct Berlin2024;

fn utc(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text).unwrap().to_utc()
}

fn offset_at(at: NaiveDateTime) -> FixedOffset {
    let summer =
        (utc("2024-03-31T01:00:00Z").naive_utc()..utc("2024-10-27T01:00:00Z").naive_utc()).contains(&at);
    FixedOffset::east_opt(if summer { 7200 } else { 3600 }).unwrap()
}

impl TimeZone for Berlin2024 {
    type Offset = FixedOffset;

    fn from_offset(_: &FixedOffset) -> Self {
        Berlin2024
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<FixedOffset> {
        self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN))
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        let [summer, winter] = [7200, 3600].map(|s| FixedOffset::east_opt(s).unwrap());
        match [summer, winter].map(|o| offset_at(*local - o) == o) {
            [true, true] => MappedLocalTime::Ambiguous(summer, winter),
            [true, false] => MappedLocalTime::Single(summer),
            [false, true] => MappedLocalTime::Single(winter),
            [false, false] => MappedLocalTime::None,
        }
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        offset_at(utc.and_time(NaiveTime::MIN))
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        offset_at(*utc)
    }
}

fn changes(from: &str, count: usize) -> Vec<(DateTime<Utc>, bool)> {
    let rule = r#"rules = [{ when = { time_range = { start = "02:30", end = "05:00" } }, then = "pause" }]"#;
    let s: Schedule = toml::from_str(rule).unwrap();
    let mut at = utc(from).with_timezone(&Berlin2024);
    (0..count)
        .map(|_| {
            at = s.evaluate(&at, false).next_change.unwrap();
            (at.to_utc(), s.evaluate(&at, false).actions == [Action::Pause])
        })
        .collect()
}

#[test]
fn test_zone_has_a_gap_and_a_repeated_hour() {
    assert_eq!(Berlin2024.with_ymd_and_hms(2024, 3, 31, 2, 30, 0), MappedLocalTime::None);
    assert!(matches!(Berlin2024.with_ymd_and_hms(2024, 10, 27, 2, 30, 0), MappedLocalTime::Ambiguous(..)));
}

#[test]
fn range_starting_in_the_spring_gap_begins_when_the_gap_ends() {
    let expected = [(utc("2024-03-31T01:00:00Z"), true), (utc("2024-03-31T03:00:00Z"), false)];
    assert_eq!(changes("2024-03-30T23:00:00Z", 2), expected);
}

#[test]
fn repeated_autumn_hour_is_followed_on_the_wall_clock() {
    let expected = [
        (utc("2024-10-27T00:30:00Z"), true),
        (utc("2024-10-27T01:00:00Z"), false),
        (utc("2024-10-27T01:30:00Z"), true),
        (utc("2024-10-27T04:00:00Z"), false),
    ];
    assert_eq!(changes("2024-10-26T22:00:00Z", 4), expected);
}
