use chrono::{DateTime, FixedOffset, NaiveDate, TimeDelta, TimeZone, Utc};
use wprs_schedule::{Location, Sun};

const CAPE_TOWN: Location = Location { latitude: -33.9249, longitude: 18.4241 };
const MADRID: Location = Location { latitude: 40.4168, longitude: -3.7038 };
const REYKJAVIK: Location = Location { latitude: 64.1466, longitude: -21.9426 };
const TROMSO: Location = Location { latitude: 69.6492, longitude: 18.9553 };

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn utc(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text).unwrap().to_utc()
}

fn assert_near(actual: DateTime<Utc>, expected: &str) {
    let error = (actual - utc(expected)).abs();
    assert!(error <= TimeDelta::minutes(2), "{actual} is {error} away from {expected}");
}

fn assert_sun(location: Location, day: NaiveDate, rise: &str, set: &str) {
    let Sun::Rises { rise: r, set: s } = location.sun(day) else { panic!("no sunrise on {day}") };
    assert_near(r, rise);
    assert_near(s, set);
}

#[test]
fn cape_town_winter_solstice() {
    assert_sun(CAPE_TOWN, date(2024, 6, 21), "2024-06-21T07:51:00+02:00", "2024-06-21T17:45:00+02:00");
}

#[test]
fn madrid_summer_spring_and_winter() {
    assert_sun(MADRID, date(2024, 6, 21), "2024-06-21T06:45:00+02:00", "2024-06-21T21:48:00+02:00");
    assert_sun(MADRID, date(2025, 5, 22), "2025-05-22T06:52:00+02:00", "2025-05-22T21:30:00+02:00");
    assert_sun(MADRID, date(2024, 12, 21), "2024-12-21T08:34:00+01:00", "2024-12-21T17:51:00+01:00");
}

#[test]
fn reykjavik_sets_after_midnight_in_june() {
    assert_sun(REYKJAVIK, date(2024, 6, 21), "2024-06-21T02:55:00Z", "2024-06-22T00:04:00Z");
    assert_sun(REYKJAVIK, date(2024, 12, 21), "2024-12-21T11:22:00Z", "2024-12-21T15:29:00Z");
    assert!(REYKJAVIK.is_day(utc("2024-06-21T23:59:00Z")));
    assert!(!REYKJAVIK.is_day(utc("2024-06-22T01:00:00Z")));
    assert_near(REYKJAVIK.next_sun_change(utc("2024-06-21T23:00:00Z")).unwrap(), "2024-06-22T00:04:00Z");
}

#[test]
fn tromso_polar_night_and_midnight_sun() {
    assert_eq!(TROMSO.sun(date(2024, 12, 21)), Sun::PolarNight);
    assert_eq!(TROMSO.sun(date(2025, 12, 4)), Sun::PolarNight);
    assert_eq!(TROMSO.sun(date(2024, 6, 21)), Sun::PolarDay);
    assert!(matches!(TROMSO.sun(date(2024, 11, 20)), Sun::Rises { .. }));
    assert!(matches!(TROMSO.sun(date(2025, 1, 25)), Sun::Rises { .. }));
    assert!(!TROMSO.is_day(utc("2024-12-21T11:00:00Z")));
    assert!(TROMSO.is_day(utc("2024-06-21T23:00:00Z")));
}

#[test]
fn polar_periods_end_on_the_expected_days() {
    let first_sunrise = TROMSO.next_sun_change(utc("2024-12-21T12:00:00Z")).unwrap();
    assert!((date(2025, 1, 13)..=date(2025, 1, 17)).contains(&first_sunrise.date_naive()), "{first_sunrise}");
    let first_sunset = TROMSO.next_sun_change(utc("2024-06-21T12:00:00Z")).unwrap();
    assert!((date(2024, 7, 22)..=date(2024, 7, 27)).contains(&first_sunset.date_naive()), "{first_sunset}");
    assert!(!TROMSO.is_day(first_sunset));
    assert!(TROMSO.is_day(first_sunset - TimeDelta::seconds(1)));
}

#[test]
fn changes_alternate_through_a_year() {
    let tz = FixedOffset::east_opt(2 * 3600).unwrap();
    let start = tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap().to_utc();
    for (location, max_gap) in [(CAPE_TOWN, 16), (TROMSO, 24 * 80)] {
        let mut at = start;
        while at < start + TimeDelta::days(365) {
            let next = location.next_sun_change(at).unwrap();
            assert!(next > at && next - at < TimeDelta::hours(max_gap), "{at} -> {next}");
            assert_ne!(location.is_day(next), location.is_day(at), "{next}");
            at = next;
        }
    }
}
