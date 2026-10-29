use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
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
    assert_eq!(c.options().len(), 16);
    assert!(!c.playback.flash_guard);
    c.set_option("playback.flash_guard", "true").unwrap();
    assert!(c.playback.sanitized().flash_guard);
    c.set_option("library.hide_mature", "true").unwrap();
    assert!(c.library.hide_mature);
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
fn hex_colors() {
    assert_eq!(parse_hex_color("#ff0080"), Some([255, 0, 128]));
    assert_eq!(parse_hex_color("ff0080"), Some([255, 0, 128]));
    assert_eq!(parse_hex_color("#fff"), None);
    assert_eq!(parse_hex_color("#gg0000"), None);
}

#[test]
fn uri_schemes() {
    let scheme = wprs_core::dbus::uri_scheme;
    assert_eq!(scheme("http://127.0.0.1:9/x.webm"), Some("http"));
    assert_eq!(scheme("file:///tmp/a.png"), Some("file"));
    assert_eq!(scheme("svn+ssh:x"), Some("svn+ssh"));
    assert_eq!(scheme("/tmp/a:b.png"), None);
    assert_eq!(scheme("a b:c.png"), None);
    assert_eq!(scheme("1x:y"), None);
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
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

#[test]
fn a_fit_for_every_output_clears_per_output_fits() {
    let mut c = Config::default();
    c.set_image("DP-1", "/b.png".into(), Some(Fit::Tile));
    c.set_image("", "/a.png".into(), Some(Fit::Center));
    assert_eq!(c.resolve("DP-1"), c.resolve("HDMI-A-1"));
    assert_eq!(c.resolve("DP-1").fit, Fit::Center);
}

#[test]
fn fit_only_changes_keep_the_images() {
    let mut c = Config::default();
    c.set_image("", "/a.png".into(), None);
    c.set_image("DP-1", "/b.png".into(), Some(Fit::Tile));
    c.set_fit("HDMI-A-1", Fit::Stretch);
    let hdmi = c.resolve("HDMI-A-1");
    assert_eq!((hdmi.image, hdmi.fit), (Some(PathBuf::from("/a.png")), Fit::Stretch));
    c.set_fit("", Fit::Center);
    let dp = c.resolve("DP-1");
    assert_eq!((dp.image, dp.fit), (Some(PathBuf::from("/b.png")), Fit::Center));
    assert_eq!(c.resolve("HDMI-A-1").fit, Fit::Center);
}

#[test]
fn saving_through_a_symlink_keeps_the_link() {
    let dir = tempfile::tempdir().unwrap();
    let (real, link) = (dir.path().join("shared/config.toml"), dir.path().join("config/config.toml"));
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink("../shared/config.toml", &link).unwrap();
    let mut c = Config::default();
    for fit in [Fit::Center, Fit::Tile] {
        c.set_fit("", fit);
        c.save(&link).unwrap();
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(Config::load(&real).unwrap(), c);
    }
    assert_eq!(std::fs::read_dir(real.parent().unwrap()).unwrap().count(), 1);
}

#[test]
fn invalid_colors_are_rejected_when_set_and_ignored_when_loaded() {
    let mut c = Config::default();
    assert!(c.set_background("", "red").is_err());
    c.set_background("", "#A0B0C0").unwrap();
    c.set_background("DP-1", "102030").unwrap();
    assert_eq!(
        (c.default.background.as_deref(), c.resolve("DP-1").background),
        (Some("#a0b0c0"), [16, 32, 48])
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[default]\nbackground = \"#102030\"\n[outputs.DP-1]\nbackground = \"red\"\n")
        .unwrap();
    let c = Config::load(&path).unwrap();
    assert_eq!((c.resolve("DP-1").background, c.outputs["DP-1"].background.as_deref()), ([16, 32, 48], None));
}

#[test]
fn configuration_files_are_private() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallpaper-rs/config.toml");
    Config::default().save(&path).unwrap();
    assert_eq!((mode(&path), mode(path.parent().unwrap())), (0o600, 0o700));
    std::fs::write(&path, "[default").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Config::load(&path).is_err());
    let backup = Config::backup_broken(&path).unwrap();
    assert_eq!((backup.file_name().unwrap().to_str(), mode(&backup)), (Some("config.toml.broken"), 0o600));
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "[default");
    Config::default().save(&path).unwrap();
    assert_eq!(mode(&path), 0o600);
}
