use std::time::Duration;
use wprs_core::{Config, Playback};

#[test]
fn settings_sections() {
    let c: Config = toml::from_str("[default]\nimage = \"/a.mp4\"").unwrap();
    assert_eq!(
        (c.playback.clone(), c.transition.duration()),
        (Playback::default(), Duration::from_millis(600))
    );
    let c: Config =
        toml::from_str("[playback]\nvolume = 2.0\nmax_fps = 900\n[transition]\nkind = \"none\"").unwrap();
    assert_eq!((c.playback.sanitized().volume, c.playback.sanitized().max_fps), (1.0, 240));
    let display = |text: &str| toml::from_str::<Config>(text).map(|c| c.playback.max_fps).ok();
    assert_eq!(display("[playback]\nmax_fps = \"display\""), Some(0));
    assert_eq!(display("[playback]\nmax_fps = 0"), Some(0));
    assert_eq!(display("[playback]\nmax_fps = \"fast\""), None);
    assert_eq!(c.transition.duration(), Duration::ZERO);
    assert!(toml::from_str::<Config>("[transition]\nkind = \"spin\"").is_err());
}

#[test]
fn options_are_listed_and_validated() {
    let mut c = Config::default();
    assert_eq!(c.options().len(), 21);
    assert!(
        !c.playback.flash_guard && c.playback.scene_sound && c.playback.scene_capture && !c.playback.duck
    );
    c.set_option("playback.duck", "true").unwrap();
    assert!(c.playback.duck && c.set_option("playback.duck", "often").is_err());
    c.set_option("playback.scene_capture", "false").unwrap();
    assert!(!c.playback.sanitized().scene_capture && c.playback.scene_sound);
    c.set_option("playback.flash_guard", "true").unwrap();
    assert!(c.playback.sanitized().flash_guard);
    c.set_option("library.hide_mature", "true").unwrap();
    c.set_option("library.show_mature", "true").unwrap();
    assert!(c.library.hide_mature && c.library.show_mature);
    c.set_option("models.allow_download", "false").unwrap();
    c.set_option("playback.software_parallax", "true").unwrap();
    assert!(!c.models.allow_download && c.playback.sanitized().software_parallax);
    c.set_option("layout.bezel_mm", "8.5").unwrap();
    assert!(toml::to_string(&c).unwrap().contains("[layout]\nbezel_mm = 8.5"));
    c.set_option("pause.on_battery", "false").unwrap();
    c.set_option("transition.kind", "none").unwrap();
    c.set_option("playback.max_fps", "0").unwrap();
    assert!(toml::to_string(&c).unwrap().contains("max_fps = \"display\""));
    c.set_option("playback.max_fps", "60").unwrap();
    assert_eq!(c.options()["playback.max_fps"], toml::Value::Integer(60));
    c.set_option("playback.max_fps", "display").unwrap();
    assert_eq!(c.playback.max_fps, 0);
    c.set_option("playback.max_fps", "60").unwrap();
    c.set_option("playback.volume", "0.25").unwrap();
    assert!(!c.pause.on_battery && c.transition.duration().is_zero());
    assert_eq!((c.playback.max_fps, c.playback.volume), (60, 0.25));
    let invalid = [
        ("playback.max_fps", "241"),
        ("playback.max_fps", "fast"),
        ("library.thumb_cache_mb", "big"),
        ("playback.volume", "3"),
        ("layout.bezel_mm", "-1"),
        ("layout.bezel_mm", "nan"),
        ("layout.monitors", "x"),
        ("transition.kind", "spin"),
        ("pause.on_battery", "maybe"),
        ("default.image", "/x"),
        ("nope", "1"),
    ];
    invalid.iter().for_each(|(k, v)| assert!(c.set_option(k, v).is_err(), "{k} = {v}"));
    let typo = c.set_option("transition.duration_ms", "abc").unwrap_err();
    assert_eq!(typo, "invalid value for transition.duration_ms: invalid digit found in string");
    assert_eq!((c.playback.max_fps, c.layout.bezel_mm), (60, 8.5));
    c.set_option("layout.bezel_mm", "0").unwrap();
    assert!(c.layout.is_default());
}

#[test]
fn hand_edited_values_are_clamped_and_only_the_changed_key_is_validated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[playback]\nvolume = 2.0\nmax_fps = 900\n[transition]\nduration_ms = 20000\n")
        .unwrap();
    let mut c = Config::load(&path).unwrap();
    assert_eq!((c.playback.volume, c.playback.max_fps, c.transition.duration_ms), (1.0, 240, 10_000));
    c.set_option("pause.on_battery", "false").unwrap();
    let mut raw: Config = toml::from_str("[playback]\nvolume = 2.0").unwrap();
    raw.set_option("playback.muted", "false").unwrap();
    let error = raw.set_option("transition.duration_ms", "20000").unwrap_err();
    assert!(error.contains("transition.duration_ms"), "{error}");
}
