use std::path::Path;
use std::time::Duration;
use wprs_core::config::parse_hex_color;
use wprs_core::{Config, Fit, Playback, UvRect};

fn approx(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
}

#[test]
fn fit_names_roundtrip() {
    Fit::ALL.into_iter().for_each(|f| assert_eq!(f.as_str().parse::<Fit>().unwrap(), f));
    assert_eq!(" FILL ".parse::<Fit>().unwrap(), Fit::Fill);
    assert!("zoom".parse::<Fit>().is_err());
}

#[test]
fn uv_mapping_per_fit() {
    let uv = UvRect::compute;
    assert!(approx(uv(Fit::Stretch, (100, 50), (1920, 1080)).scale, [1.0, 1.0]));
    assert!(approx(uv(Fit::Fill, (1280, 720), (1920, 1080)).scale, [1.0, 1.0]));
    let fill = uv(Fit::Fill, (100, 100), (200, 100));
    assert!(approx(fill.scale, [1.0, 0.5]) && approx(fill.offset, [0.0, 0.25]));
    let fit = uv(Fit::Fit, (100, 100), (200, 100));
    assert!(approx(fit.scale, [2.0, 1.0]) && approx(fit.offset, [-0.5, 0.0]));
    let center = uv(Fit::Center, (100, 50), (200, 100));
    assert!(approx(center.scale, [2.0, 2.0]) && approx(center.offset, [-0.5, -0.5]) && !center.repeat);
    let tile = uv(Fit::Tile, (100, 50), (200, 100));
    assert!(approx(tile.scale, [2.0, 2.0]) && approx(tile.offset, [0.0, 0.0]) && tile.repeat);
    assert!(uv(Fit::Fill, (0, 0), (0, 0)).scale.iter().all(|s| s.is_finite()));
}

#[test]
fn missing_file_is_default() {
    assert_eq!(Config::load(Path::new("/nonexistent/wallpaper-rs/config.toml")).unwrap(), Config::default());
}

#[test]
fn resolve_falls_back_per_field() {
    let text = "[default]\nimage = \"/a.png\"\nfit = \"fit\"\nbackground = \"#102030\"\n[outputs.DP-1]\nimage = \"/b.png\"";
    let c: Config = toml::from_str(text).unwrap();
    let dp = c.resolve("DP-1");
    assert_eq!(
        (dp.image.as_deref(), dp.fit, dp.background),
        (Some(Path::new("/b.png")), Fit::Fit, [16, 32, 48])
    );
    assert_eq!(c.resolve("HDMI-A-1").image.as_deref(), Some(Path::new("/a.png")));
    assert!(toml::from_str::<Config>("[default]\nimgae = \"/x\"").is_err());
}

#[test]
fn setting_every_output_clears_overrides() {
    let mut c = Config::default();
    c.set_image("DP-1", "/b.png".into(), None);
    c.set_image("", "/a.png".into(), Some(Fit::Center));
    let dp = c.resolve("DP-1");
    assert_eq!((dp.image.as_deref(), dp.fit), (Some(Path::new("/a.png")), Fit::Center));
}

#[test]
fn save_and_load_roundtrip() {
    let path =
        std::env::temp_dir().join(format!("wprs-core-test-{}", std::process::id())).join("config.toml");
    let mut c = Config::default();
    c.set_image("DP-2", "/c.png".into(), Some(Fit::Tile));
    c.save(&path).unwrap();
    assert_eq!(Config::load(&path).unwrap(), c);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn settings_sections() {
    let c: Config = toml::from_str("[default]\nimage = \"/a.mp4\"").unwrap();
    assert_eq!(
        (c.playback.clone(), c.transition.duration()),
        (Playback::default(), Duration::from_millis(600))
    );
    let c: Config =
        toml::from_str("[playback]\nvolume = 2.0\nmax_fps = 0\n[transition]\nkind = \"none\"").unwrap();
    assert_eq!((c.playback.sanitized().volume, c.playback.sanitized().max_fps), (1.0, 1));
    assert_eq!(c.transition.duration(), Duration::ZERO);
    assert!(toml::from_str::<Config>("[transition]\nkind = \"spin\"").is_err());
}

#[test]
fn options_are_listed_and_validated() {
    let mut c = Config::default();
    assert_eq!(c.options().len(), 8);
    c.set_option("pause.on_battery", "false").unwrap();
    c.set_option("transition.kind", "none").unwrap();
    c.set_option("playback.max_fps", "60").unwrap();
    c.set_option("playback.volume", "0.25").unwrap();
    assert!(!c.pause.on_battery && c.transition.duration().is_zero());
    assert_eq!((c.playback.max_fps, c.playback.volume), (60, 0.25));
    let invalid = [
        ("playback.max_fps", "0"),
        ("playback.volume", "3"),
        ("transition.kind", "spin"),
        ("pause.on_battery", "maybe"),
        ("default.image", "/x"),
        ("nope", "1"),
    ];
    invalid.iter().for_each(|(k, v)| assert!(c.set_option(k, v).is_err(), "{k} = {v}"));
    assert_eq!(c.playback.max_fps, 60);
}

#[test]
fn hex_colors() {
    assert_eq!(parse_hex_color("#ff0080"), Some([255, 0, 128]));
    assert_eq!(parse_hex_color("ff0080"), Some([255, 0, 128]));
    assert_eq!(parse_hex_color("#fff"), None);
    assert_eq!(parse_hex_color("#gg0000"), None);
}
