use wprs_core::{Config, QualityChoice};

#[test]
fn the_quality_setting_takes_auto_a_fixed_level_or_a_floor() {
    let mut c = Config::default();
    assert_eq!(c.options()["playback.quality"], toml::Value::String("auto".into()));
    assert_eq!(c.quality("DP-1"), QualityChoice::Auto);
    c.set_option("playback.quality", " Q3 ").unwrap();
    assert_eq!(c.playback.quality, QualityChoice::Fixed(3));
    c.set_option("playback.quality", "min-q1").unwrap();
    assert_eq!((c.playback.quality, c.playback.quality.floor()), (QualityChoice::AtLeast(1), 1));
    let saved: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(saved.playback.quality, QualityChoice::AtLeast(1));
    for bad in ["", "q4", "min-q0", "min-q3", "q01", "q+1", "high", "minq1"] {
        let error = c.set_option("playback.quality", bad).unwrap_err();
        assert!(
            error.starts_with("invalid value for playback.quality") && error.contains("min-q2"),
            "{bad:?}"
        );
    }
    assert_eq!(c.playback.quality, QualityChoice::AtLeast(1), "a refused value changes nothing");
    let words = QualityChoice::WORDS.map(|w| QualityChoice::parse(w).unwrap().as_str());
    assert_eq!(words, QualityChoice::WORDS);
    assert!(toml::from_str::<Config>("[playback]\nquality = 2").is_err());
    assert!(toml::from_str::<Config>("[playback]\nquality = \"best\"").is_err());
}

#[test]
fn a_screen_can_keep_its_own_quality_and_drop_it_again() {
    let mut c = Config::default();
    c.set_option("playback.quality", "min-q2").unwrap();
    c.set_option("playback.quality.DP-1", "q0").unwrap();
    assert_eq!(
        (c.quality("DP-1"), c.quality("HDMI-A-1")),
        (QualityChoice::Fixed(0), QualityChoice::AtLeast(2))
    );
    assert_eq!(c.quality(""), QualityChoice::AtLeast(2), "every screen follows the general setting");
    let text = toml::to_string(&c).unwrap();
    assert!(text.contains("[outputs.DP-1]\nquality = \"q0\""), "{text}");
    let saved: Config = toml::from_str(&text).unwrap();
    assert_eq!(saved.quality("DP-1"), QualityChoice::Fixed(0));
    assert!(c.set_option("playback.quality.DP-1", "q9").is_err());
    assert!(c.set_option("playback.quality.", "q1").is_err());
    assert!(c.set_option("playback.quality.a\u{7}b", "q1").is_err());
    assert_eq!(c.quality("DP-1"), QualityChoice::Fixed(0));
    assert!(!c.options().keys().any(|k| k.starts_with("playback.quality.")), "own levels are not listed");
    c.set_option("playback.quality.DP-1", "none").unwrap();
    assert_eq!(c.quality("DP-1"), QualityChoice::AtLeast(2));
    assert!(c.outputs.is_empty(), "an empty screen section is dropped");
    c.set_option("playback.quality.HDMI-A-1", "").unwrap();
    assert!(c.outputs.is_empty());
}

#[test]
fn configurations_without_the_setting_stay_automatic() {
    let old: Config = toml::from_str("[playback]\nmax_fps = 30\n[outputs.DP-1]\nfit = \"fit\"\n").unwrap();
    assert_eq!((old.playback.quality, old.quality("DP-1")), (QualityChoice::Auto, QualityChoice::Auto));
    assert!(!toml::to_string(&old).unwrap().contains("[outputs.DP-1]\nquality"));
}
