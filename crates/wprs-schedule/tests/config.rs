use chrono::{NaiveTime, Weekday};
use std::time::Duration;
use wprs_schedule::{Action, Condition, Location, Order, Rule, Schedule, ScheduleError, parse_interval};

const EXAMPLE: &str = r#"
location = { latitude = -34.6037, longitude = -58.3816 }

[[playlists]]
id = "day"
items = ["/w/a.png", "/w/b.png", "/w/c.mp4"]
order = "shuffle"
interval = "1h30m"
per_output = true

[[playlists]]
id = "night"
items = ["/w/night.png"]
interval = "45s"

[[rules]]
when = { time_range = { start = "22:00", end = "06:30" } }
then = { show_playlist = "night" }

[[rules]]
when = { all = [{ weekdays = ["sat", "Sunday"] }, "daylight"] }
then = { show_file = "/w/weekend.png" }

[[rules]]
when = { on_battery = true }
then = { set_max_fps = 20 }

[[rules]]
when = "night"
then = "pause"
"#;

fn parse(text: &str) -> Schedule {
    toml::from_str(text).unwrap()
}

#[test]
fn parses_the_example() {
    let s = parse(EXAMPLE);
    s.validate().unwrap();
    assert_eq!(s.location, Some(Location { latitude: -34.6037, longitude: -58.3816 }));
    let (day, night) = (s.playlist("day").unwrap(), s.playlist("night").unwrap());
    assert_eq!((day.order, day.interval, day.per_output), (Order::Shuffle, Duration::from_secs(5400), true));
    assert_eq!(
        (night.order, night.interval, night.per_output),
        (Order::Sequential, Duration::from_secs(45), false)
    );
    let time = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap();
    assert_eq!(s.rules[0].when, Condition::TimeRange { start: time(22, 0), end: time(6, 30) });
    let weekend =
        Condition::All(vec![Condition::Weekdays(vec![Weekday::Sat, Weekday::Sun]), Condition::Daylight]);
    assert_eq!(s.rules[1], Rule { when: weekend, then: Action::ShowFile("/w/weekend.png".into()) });
    assert_eq!((&s.rules[2].then, &s.rules[3].then), (&Action::SetMaxFps(20), &Action::Pause));
}

#[test]
fn roundtrips_through_toml_and_json() {
    let s = parse(EXAMPLE);
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("interval = \"1h30m\""), "{text}");
    assert_eq!(parse(&text), s);
    assert_eq!(serde_json::from_str::<Schedule>(&serde_json::to_string(&s).unwrap()).unwrap(), s);
    assert_eq!(parse(""), Schedule::default());
}

#[test]
fn intervals_use_unit_suffixes() {
    assert_eq!(parse_interval("1d 2h 3m 4s"), Some(Duration::from_secs(93_784)));
    assert_eq!(parse_interval("90s"), Some(Duration::from_secs(90)));
    for bad in ["10", "5x", "", "m", "1h 30", "-1m"] {
        assert_eq!(parse_interval(bad), None, "{bad:?}");
    }
    assert_eq!(wprs_schedule::format_interval(Duration::from_secs(5400)), "1h30m");
}

#[test]
fn rejects_bad_input() {
    let playlist = "[[playlists]]\nid = \"a\"\nitems = [\"/a\"]\ninterval = \"1m\"\n";
    assert!(toml::from_str::<Schedule>(&playlist.replace("1m", "1")).is_err());
    assert!(toml::from_str::<Schedule>(&format!("{playlist}shufle = true")).is_err());
    assert!(toml::from_str::<Schedule>("[[rules]]\nwhen = \"sometimes\"\nthen = \"pause\"").is_err());
    assert!(
        toml::from_str::<Schedule>("rules = [{ when = { weekdays = [\"funday\"] }, then = \"pause\" }]")
            .is_err()
    );
    let missing = parse("[[rules]]\nwhen = \"night\"\nthen = { show_playlist = \"x\" }");
    assert_eq!(missing.validate(), Err(ScheduleError::UnknownPlaylist(1, "x".into())));
    let sun = parse("[[rules]]\nwhen = { all = [\"daylight\"] }\nthen = \"pause\"");
    assert_eq!(sun.validate(), Err(ScheduleError::MissingLocation(1)));
    let twice = parse(&format!("{playlist}{playlist}"));
    assert_eq!(twice.validate(), Err(ScheduleError::DuplicatePlaylist("a".into())));
    let empty = parse(&playlist.replace("[\"/a\"]", "[]"));
    assert_eq!(empty.validate(), Err(ScheduleError::EmptyPlaylist("a".into())));
    let far = parse("location = { latitude = 91.0, longitude = 0.0 }");
    assert_eq!(far.validate(), Err(ScheduleError::InvalidLocation(91.0, 0.0)));
}
