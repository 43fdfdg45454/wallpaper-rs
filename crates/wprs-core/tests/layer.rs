use wprs_core::layer::{Refused, Source};
use wprs_core::model::LayerShellState;
use wprs_core::{Config, LayerNamespace};

const KEY: &str = LayerNamespace::KEY;

fn named(text: &str) -> LayerNamespace {
    LayerNamespace::parse(text).unwrap().unwrap()
}

#[test]
fn the_namespace_setting_is_checked_and_left_out_until_set() {
    let mut c = Config::default();
    assert_eq!(c.options()[KEY], toml::Value::String("auto".into()));
    assert!(!toml::to_string(&c).unwrap().contains("surfaces"), "an untouched configuration stays as it was");
    c.set_option(KEY, " desktop-bg ").unwrap();
    assert_eq!(c.surfaces.layer_namespace, Some(named("desktop-bg")));
    assert!(toml::to_string(&c).unwrap().contains("[surfaces]\nlayer_namespace = \"desktop-bg\""));
    let saved: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(saved.surfaces.layer_namespace, Some(named("desktop-bg")));
    let long = "x".repeat(LayerNamespace::MAX + 1);
    for bad in ["two words", "a/b", "ünï", "a\u{7}b", long.as_str()] {
        let error = c.set_option(KEY, bad).unwrap_err();
        assert!(
            error.starts_with("invalid value for surfaces.layer_namespace") && error.contains("auto"),
            "{bad:?}: {error}"
        );
    }
    assert_eq!(c.surfaces.layer_namespace, Some(named("desktop-bg")), "a refused value changes nothing");
    assert!(LayerNamespace::parse(&"x".repeat(LayerNamespace::MAX)).unwrap().is_some());
    assert!(toml::from_str::<Config>("[surfaces]\nlayer_namespace = 3").is_err());
    assert!(toml::from_str::<Config>("[surfaces]\nlayer_namespace = \"a b\"").is_err());
    assert!(toml::from_str::<Config>("[surfaces]\nnamespace = \"a\"").is_err());
    let auto: Config = toml::from_str("[surfaces]\nlayer_namespace = \"Auto\"").unwrap();
    assert!(auto.surfaces.is_default());
    for clear in ["auto", "", "  "] {
        c.set_option(KEY, "desktop-bg").unwrap();
        c.set_option(KEY, clear).unwrap();
        assert!(c.surfaces.is_default() && !toml::to_string(&c).unwrap().contains("surfaces"), "{clear:?}");
    }
}

#[test]
fn the_option_wins_over_the_environment_the_file_and_the_desktop() {
    let file = named("from-file");
    let choose = |o, e, c, d| {
        let chosen = LayerNamespace::choose(o, e, c, d);
        (chosen.name, chosen.from)
    };
    let both = (Some("from-option"), Some("from-env"));
    assert_eq!(choose(both.0, both.1, Some(&file), "KineticWE"), ("from-option".into(), Source::Option));
    assert_eq!(choose(None, both.1, Some(&file), "KineticWE"), ("from-env".into(), Source::Environment));
    assert_eq!(choose(None, None, Some(&file), "KineticWE"), ("from-file".into(), Source::Config));
    assert_eq!(choose(None, None, None, "KineticWE"), ("noctalia-wallpaper".into(), Source::Automatic));
    assert_eq!(choose(None, None, None, "sway"), ("wallpaper".into(), Source::Default));
    assert_eq!(choose(None, None, None, ""), ("wallpaper".into(), Source::Default));
    assert_eq!(choose(Some("auto"), Some(""), None, "sway"), ("wallpaper".into(), Source::Default));
    let ignored = LayerNamespace::choose(None, Some("bad name"), Some(&file), "");
    assert_eq!((ignored.name.as_str(), ignored.from), ("from-file", Source::Config));
    assert!(ignored.ignored.is_some_and(|e| e.contains("environment")));
    assert_eq!(ignored.refused, Some(Refused { from: Source::Environment, empty: false }));
    let option = LayerNamespace::choose(Some("a/b"), Some("fine"), None, "");
    assert_eq!(
        (option.name.as_str(), option.refused),
        ("fine", Some(Refused { from: Source::Option, empty: false }))
    );
    assert_eq!(LayerNamespace::choose(None, None, None, "").refused, None);
    assert_eq!(LayerNamespace::choose(None, None, None, "").ignored, None);
    for blank in ["", "  "] {
        let set_but_blank = LayerNamespace::choose(None, Some(blank), Some(&file), "KineticWE");
        assert_eq!(
            (set_but_blank.name.as_str(), set_but_blank.from),
            ("from-file", Source::Config),
            "{blank:?}"
        );
        assert!(
            set_but_blank.ignored.is_some_and(|e| e.contains("environment") && e.contains("empty")),
            "{blank:?}"
        );
        assert_eq!(set_but_blank.refused, Some(Refused { from: Source::Environment, empty: true }));
        assert!(LayerNamespace::given(blank).unwrap_err().contains("empty"));
    }
    assert_eq!(LayerNamespace::given(" Auto ").unwrap(), None);
    assert_eq!(LayerNamespace::given("desk").unwrap(), Some(named("desk")));
    let words = [Source::Option, Source::Environment, Source::Config, Source::Automatic, Source::Default];
    assert_eq!(words.map(Source::as_str), ["option", "environment", "config", "automatic", "default"]);
}

#[test]
fn the_state_reads_a_daemon_that_does_not_report_ignored_values() {
    let old: LayerShellState =
        serde_json::from_str(r#"{"namespace": "wallpaper", "from": "default", "restart": false}"#).unwrap();
    assert_eq!((&old.ignored, &old.ignored_from, old.ignored_empty), (&None, &None, false));
    let new = LayerShellState {
        ignored: Some("the environment value is ignored".into()),
        ignored_from: Some("environment".into()),
        ignored_empty: true,
        ..old
    };
    let back: LayerShellState = serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
    assert_eq!(back, new);
}

#[test]
fn known_desktops_choose_their_namespace() {
    assert_eq!(LayerNamespace::automatic("KineticWE"), Some("noctalia-wallpaper"));
    assert_eq!(LayerNamespace::automatic("kineticwe"), Some("noctalia-wallpaper"));
    assert_eq!(LayerNamespace::automatic("X-Generic:KineticWE"), Some("noctalia-wallpaper"));
    for other in ["", "sway", "KDE", "Hyprland", "KineticWEish", "X-KineticWE"] {
        assert_eq!(LayerNamespace::automatic(other), None, "{other}");
    }
}
