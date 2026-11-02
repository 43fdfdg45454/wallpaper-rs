use wprs_core::Config;
use wprs_core::rgb::{Rgb, SERVER, valid};

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
