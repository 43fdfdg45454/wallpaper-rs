use wprs_core::{Config, GpuChoice};

#[test]
fn the_graphics_card_setting_takes_words_or_part_of_a_name() {
    let mut c = Config::default();
    assert_eq!(c.options()["render.gpu"], toml::Value::String("auto".into()));
    assert!(!toml::to_string(&c).unwrap().contains("[render]"), "an untouched configuration stays as it was");
    c.set_option("render.gpu", "Dedicated").unwrap();
    assert_eq!(c.render.gpu, GpuChoice::Dedicated);
    assert!(toml::to_string(&c).unwrap().contains("[render]\ngpu = \"dedicated\""));
    c.set_option("render.gpu", "  RTX 5070 ").unwrap();
    assert_eq!(c.render.gpu, GpuChoice::Named("RTX 5070".into()));
    let saved: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(saved.render.gpu, GpuChoice::Named("RTX 5070".into()));
    for bad in ["", "   ", "a\u{7}b", &"x".repeat(GpuChoice::MAX_NAME + 1)] {
        let error = c.set_option("render.gpu", bad).unwrap_err();
        assert!(
            error.starts_with("invalid value for render.gpu") && error.contains("auto"),
            "{bad:?}: {error}"
        );
    }
    assert_eq!(c.render.gpu, GpuChoice::Named("RTX 5070".into()), "a refused value changes nothing");
    assert!(toml::from_str::<Config>("[render]\ngpu = 3").is_err());
    assert!(toml::from_str::<Config>("[render]\ngpu = \"\"").is_err());
    assert!(toml::from_str::<Config>("[render]\ncard = \"auto\"").is_err());
    c.set_option("render.gpu", "auto").unwrap();
    assert!(c.render.is_default() && !toml::to_string(&c).unwrap().contains("render"));
    let words = GpuChoice::WORDS.map(|w| GpuChoice::parse(&w.to_uppercase()).unwrap());
    assert_eq!(words.map(|w| w.as_str().to_owned()), GpuChoice::WORDS.map(String::from));
    assert_eq!(GpuChoice::parse("software").unwrap(), GpuChoice::Software);
    let named = |text: &str| GpuChoice::parse(text).unwrap();
    assert!(named("RTX 5070").same(&named("rtx 5070")), "names match whatever their letter case");
    assert!(named("Dedicated").same(&GpuChoice::Dedicated));
    assert!(!named("RTX 5070").same(&named("RTX 4060")) && !GpuChoice::Auto.same(&GpuChoice::Dedicated));
}
