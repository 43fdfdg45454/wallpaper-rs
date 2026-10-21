use chrono::{DateTime, FixedOffset, TimeDelta};
use wprs_schedule::{Action, Schedule};

fn schedule(rules: &str) -> Schedule {
    toml::from_str(&format!("location = {{ latitude = 40.4168, longitude = -3.7038 }}\nrules = [{rules}]"))
        .unwrap()
}

fn at(text: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(text).unwrap()
}

fn show(id: &str) -> Action {
    Action::ShowPlaylist(id.into())
}

const NIGHT: &str =
    r#"{ when = { time_range = { start = "22:00", end = "06:30" } }, then = { show_playlist = "night" } }"#;

#[test]
fn ranges_cross_midnight() {
    let s = schedule(NIGHT);
    let active = |t: &str| s.evaluate(&at(t), false).actions == [show("night")];
    assert!(!active("2024-06-21T21:59:59+02:00"));
    assert!(active("2024-06-21T22:00:00+02:00"));
    assert!(active("2024-06-21T23:59:59+02:00"));
    assert!(active("2024-06-22T00:00:00+02:00"));
    assert!(active("2024-06-22T06:29:59+02:00"));
    assert!(!active("2024-06-22T06:30:00+02:00"));
}

#[test]
fn next_change_follows_range_edges() {
    let s = schedule(NIGHT);
    let next = |t: &str| s.evaluate(&at(t), false).next_change;
    assert_eq!(next("2024-06-21T12:00:00+02:00"), Some(at("2024-06-21T22:00:00+02:00")));
    assert_eq!(next("2024-06-21T22:00:00+02:00"), Some(at("2024-06-22T06:30:00+02:00")));
    assert_eq!(next("2024-06-22T03:00:00+02:00"), Some(at("2024-06-22T06:30:00+02:00")));
}

#[test]
fn next_change_skips_edges_that_change_nothing() {
    let s = schedule(
        r#"{ when = { all = [{ weekdays = ["sat", "sun"] }, { time_range = { start = "09:00", end = "12:00" } }] },
             then = "pause" }"#,
    );
    let friday = s.evaluate(&at("2024-06-21T10:00:00-03:00"), false);
    assert!(friday.actions.is_empty());
    assert_eq!(friday.next_change, Some(at("2024-06-22T09:00:00-03:00")));
    let sunday = s.evaluate(&at("2024-06-23T12:00:00-03:00"), false);
    assert_eq!(sunday.next_change, Some(at("2024-06-29T09:00:00-03:00")));
}

#[test]
fn first_matching_rule_wins_per_kind() {
    let s = schedule(
        r#"
        { when = { on_battery = true }, then = { set_max_fps = 15 } },
        { when = { time_range = { start = "22:00", end = "06:00" } }, then = { show_playlist = "night" } },
        { when = { time_range = { start = "00:00", end = "00:00" } }, then = { show_file = "/w/day.png" } },
        { when = { on_battery = true }, then = { set_max_fps = 60 } },
        { when = { on_battery = true }, then = "pause" },
        "#,
    );
    let night = s.evaluate(&at("2024-06-21T23:00:00Z"), true);
    assert_eq!(night.actions, [Action::SetMaxFps(15), show("night"), Action::Pause]);
    assert_eq!(night.next_change, Some(at("2024-06-22T06:00:00Z")));
    assert_eq!(
        s.evaluate(&at("2024-06-21T12:00:00Z"), false).actions,
        [Action::ShowFile("/w/day.png".into())]
    );
}

#[test]
fn daylight_follows_the_sun() {
    let s = schedule(
        r#"{ when = "daylight", then = { show_playlist = "day" } },
           { when = "night", then = { show_playlist = "night" } }"#,
    );
    let noon = s.evaluate(&at("2024-06-21T12:00:00+02:00"), false);
    assert_eq!(noon.actions, [show("day")]);
    let sunset = noon.next_change.unwrap();
    assert!((sunset - at("2024-06-21T21:48:00+02:00")).abs() < TimeDelta::minutes(2), "{sunset}");
    let evening = s.evaluate(&sunset, false);
    assert_eq!(evening.actions, [show("night")]);
    let sunrise = evening.next_change.unwrap();
    assert!((sunrise - at("2024-06-22T06:45:00+02:00")).abs() < TimeDelta::minutes(2), "{sunrise}");
}

#[test]
fn nothing_to_wait_for() {
    let battery = schedule(r#"{ when = { on_battery = true }, then = "pause" }"#);
    assert_eq!(battery.evaluate(&at("2024-06-21T12:00:00Z"), true).next_change, None);
    let shadowed = schedule(&format!(
        r#"{{ when = {{ on_battery = false }}, then = {{ show_file = "/a.png" }} }}, {NIGHT}"#
    ));
    let e = shadowed.evaluate(&at("2024-06-21T23:00:00Z"), false);
    assert_eq!(e.actions, [Action::ShowFile("/a.png".into())]);
    assert_eq!(e.next_change, None);
}
