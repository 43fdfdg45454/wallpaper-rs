use wprs_core::{AutoPace, Config};

#[test]
fn the_automatic_pace_takes_detail_or_smooth() {
    let mut c = Config::default();
    assert_eq!(c.options()["playback.auto_pace"], toml::Value::String("detail".into()));
    c.set_option("playback.auto_pace", "smooth").unwrap();
    assert_eq!(c.playback.auto_pace, AutoPace::Smooth);
    assert!(!c.playback.auto_pace.halves());
    let saved: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(saved.playback.auto_pace, AutoPace::Smooth);
    let error = c.set_option("playback.auto_pace", "fast").unwrap_err();
    assert!(error.starts_with("invalid value for playback.auto_pace"), "{error}");
    assert_eq!(c.playback.auto_pace, AutoPace::Smooth, "a refused value changes nothing");
    assert!(AutoPace::Detail.halves() && AutoPace::WORDS == ["detail", "smooth"]);
}
