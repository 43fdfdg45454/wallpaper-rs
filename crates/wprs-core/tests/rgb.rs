use wprs_core::Config;
use wprs_core::rgb::{Covered, Rgb, SERVER, valid};

#[test]
fn rgb_lighting_is_off_and_local_until_changed() {
    let mut c = Config::default();
    assert_eq!(c.rgb, Rgb::default());
    assert!(!c.rgb.enabled && c.rgb.local() && c.rgb.server == SERVER);
    assert_eq!(c.options()["rgb.enabled"], toml::Value::Boolean(false));
    c.set_option("rgb.enabled", "true").unwrap();
    c.set_option("rgb.server", "lights.example.org:6742").unwrap();
    assert!(c.rgb.enabled && !c.rgb.local());
    for local in ["localhost:6742", "[::1]:6742", "127.0.0.2:1"] {
        c.set_option("rgb.server", local).unwrap();
        assert!(c.rgb.local(), "{local}");
    }
    for bad in
        ["", "6742", "host:", "host:0", "host:70000", "a b:1", ":6742", &format!("{}:1", "a".repeat(260))]
    {
        assert!(c.set_option("rgb.server", bad).is_err(), "{bad}");
        assert!(!valid(bad), "{bad}");
    }
    assert_eq!(c.rgb.server, "127.0.0.2:1");
    assert!(c.set_option("rgb.enabled", "maybe").is_err());
}

#[test]
fn a_hand_written_bad_server_falls_back_to_this_computer() {
    let mut c: Config = toml::from_str("[rgb]\nenabled = true\nserver = \"no port\"").unwrap();
    c.normalize();
    assert_eq!((c.rgb.enabled, c.rgb.server.as_str()), (true, SERVER));
    assert!(toml::from_str::<Config>("[rgb]\ncolour = 1").is_err());
}

#[test]
fn the_lights_pause_while_covered_until_told_to_follow() {
    let mut c = Config::default();
    assert_eq!((c.rgb.when_covered, c.options()[Covered::KEY].as_str()), (Covered::Pause, Some("pause")));
    c.set_option(Covered::KEY, "follow").unwrap();
    assert_eq!(c.rgb.when_covered, Covered::Follow);
    for bad in ["", "Follow", "stop", "true", "1", "follow pause"] {
        let refused = c.set_option(Covered::KEY, bad).unwrap_err();
        assert!(refused.contains(Covered::KEY), "{bad}: {refused}");
    }
    assert_eq!(c.rgb.when_covered, Covered::Follow);
    c.set_option(Covered::KEY, " pause ").unwrap();
    assert_eq!(c.rgb.when_covered, Covered::Pause);
    let old: Config = toml::from_str("[rgb]\nenabled = true\nserver = \"127.0.0.1:1\"").unwrap();
    assert_eq!((old.rgb.enabled, old.rgb.when_covered), (true, Covered::Pause));
    let kept: Config = toml::from_str("[rgb]\nwhen_covered = \"follow\"").unwrap();
    assert!(toml::to_string(&kept).unwrap().contains("when_covered = \"follow\""));
    assert!(toml::from_str::<Config>("[rgb]\nwhen_covered = \"always\"").is_err());
    assert!(toml::from_str::<Config>("[rgb]\nwhen_covered = true").is_err());
    assert_eq!(
        Covered::WORDS.map(|w| toml::from_str::<Config>(&format!("[rgb]\nwhen_covered = \"{w}\""))
            .unwrap()
            .rgb
            .when_covered
            .as_str()),
        Covered::WORDS
    );
}

#[test]
fn devices_and_zones_are_placed_on_the_picture_and_bad_places_refused() {
    use wprs_core::rgb::{Flow, MAX_PLACES, Place};
    let mut c = Config::default();
    c.set_option("rgb.place.Example keyboard", " 0 0.5 0.5 0.5 left ").unwrap();
    c.set_option("rgb.place.Example keyboard#1", "off").unwrap();
    c.set_option("rgb.place.Example mouse", "0.75 0 0.25 1").unwrap();
    assert_eq!(
        c.rgb.place("Example keyboard"),
        Some(Place::At { rect: [0.0, 0.5, 0.5, 0.5], flow: Flow::Left })
    );
    assert_eq!(c.rgb.zone_place("Example keyboard", 1), Some(Place::Off));
    assert_eq!(c.rgb.zone_place("Example keyboard", 0), None);
    assert_eq!(c.rgb.places["Example mouse"], "0.75 0 0.25 1 right");
    assert!(!c.options().contains_key("rgb.places"), "the places were listed as a setting");
    let text = toml::to_string(&c).unwrap();
    assert!(text.contains("[rgb.places]"));
    assert_eq!(toml::from_str::<Config>(&text).unwrap().rgb, c.rgb);
    for bad in [
        "",
        "on",
        "0 0 1",
        "0 0 0 1",
        "0.5 0 0.6 1",
        "-0.1 0 0.5 1",
        "0 0 1 1 sideways",
        "0 0 1 1 right x",
        "NaN 0 1 1",
        "0 0 0.0004 1",
        "1.00005 0 0.00005 1",
        "0 1.5 1 0.5",
    ] {
        assert!(c.set_option("rgb.place.Example mouse", bad).is_err() || bad.is_empty(), "{bad:?}");
    }
    assert_eq!(c.rgb.places.get("Example mouse"), None, "an empty value did not clear the place");
    c.set_option("rgb.place.Example strip", "0.999 0 0.001 1").unwrap();
    assert_eq!(c.rgb.places["Example strip"], "0.999 0 0.001 1 right");
    assert!(c.rgb.place("Example strip").is_some(), "the smallest place cannot be read back");
    c.rgb.places.remove("Example strip");
    for bad_key in ["rgb.place.", "rgb.place.bad\u{7}name", &format!("rgb.place.{}", "d".repeat(129))] {
        assert!(c.set_option(bad_key, "off").is_err(), "{bad_key:?}");
    }
    c.set_option("rgb.place.Example keyboard", "none").unwrap();
    assert_eq!(c.rgb.place("Example keyboard"), None);
    for i in 0..MAX_PLACES {
        let _ = c.set_option(&format!("rgb.place.D{i}"), "off");
    }
    assert!(c.set_option("rgb.place.One more", "off").is_err(), "more places than allowed were kept");
    let mut read: Config =
        toml::from_str("[rgb.places]\nA = \"off\"\nB = \"2 0 1 1\"\n\"\" = \"off\"\nC = \"0 0 1 1 up\"")
            .unwrap();
    read.normalize();
    assert_eq!(read.rgb.places.keys().collect::<Vec<_>>(), ["A", "C"]);
}
