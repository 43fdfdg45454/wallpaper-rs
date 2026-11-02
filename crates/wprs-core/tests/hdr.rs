use wprs_core::hdr::{MAX_HEADROOM, UNKNOWN_HEADROOM, headroom};
use wprs_core::{Config, HdrChoice};

#[test]
fn hdr_follows_the_choice_the_display_the_power_and_the_quality() {
    use HdrChoice::*;
    assert_eq!(Auto.decide((true, true), false, true), (true, "auto"));
    assert_eq!(Auto.decide((true, true), true, true), (false, "battery"));
    assert_eq!(Auto.decide((true, true), false, false), (false, "quality"));
    assert_eq!(On.decide((true, false), true, false), (true, "on"));
    assert_eq!(On.decide((false, true), false, true), (false, "display"));
    assert_eq!(Auto.decide((false, true), false, true), (false, "display"));
    assert_eq!(Auto.decide((true, false), false, true), (false, "display"));
    assert_eq!(Auto.decide((true, false), true, false), (false, "display"));
    assert_eq!(Off.decide((true, true), false, true), (false, "off"));
    assert_eq!(HdrChoice::default(), Auto);
}

#[test]
fn the_headroom_is_the_peak_over_the_reference_white() {
    assert_eq!(headroom(Some(1000.0), Some(250.0)), 4.0);
    assert_eq!(headroom(Some(100.0), Some(203.0)), 1.0);
    assert_eq!(headroom(Some(1e9), Some(1.0)), MAX_HEADROOM);
    for (peak, reference) in [
        (None, Some(203.0)),
        (Some(1000.0), None),
        (Some(f32::NAN), Some(203.0)),
        (Some(600.0), Some(0.0)),
        (Some(-1.0), Some(80.0)),
    ] {
        assert_eq!(headroom(peak, reference), UNKNOWN_HEADROOM, "{peak:?} {reference:?}");
    }
}

#[test]
fn each_screen_may_choose_its_own_hdr_and_bad_values_are_refused() {
    let mut config = Config::default();
    assert_eq!(config.options()["playback.hdr"].as_str(), Some("auto"));
    config.set_option("playback.hdr", "off").unwrap();
    assert_eq!(config.hdr("DP-1"), HdrChoice::Off);
    config.set_option("playback.hdr.DP-1", "on").unwrap();
    assert_eq!((config.hdr("DP-1"), config.own_hdr("DP-1")), (HdrChoice::On, Some(HdrChoice::On)));
    assert_eq!(config.hdr("DP-2"), HdrChoice::Off);
    let saved = toml::to_string(&config).unwrap();
    assert!(saved.contains("hdr = \"on\"") && saved.contains("hdr = \"off\""), "{saved}");
    assert_eq!(toml::from_str::<Config>(&saved).unwrap(), config);
    config.set_option("playback.hdr.DP-1", "none").unwrap();
    assert!(config.outputs.is_empty());
    for (key, value) in [
        ("playback.hdr", "bright"),
        ("playback.hdr.DP-1", "maybe"),
        ("playback.hdr.", "on"),
        ("playback.hdr.a\nb", "on"),
    ] {
        assert!(config.set_option(key, value).is_err(), "{key}={value}");
    }
    let long = format!("playback.hdr.{}", "x".repeat(129));
    assert!(config.set_option(&long, "on").is_err());
    assert_eq!(HdrChoice::parse(" ON "), Ok(HdrChoice::On));
    assert!(HdrChoice::WORDS.iter().all(|w| HdrChoice::parse(w).unwrap().as_str() == *w));
}
