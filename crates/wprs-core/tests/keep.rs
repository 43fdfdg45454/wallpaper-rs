use wprs_core::Config;

const TEXT: &str = "profile = \"01J\"\n\n# my wallpapers\n[transition]\n# slower fades\nduration_ms = 900 # tuned by hand\n\n\
    [providers.zeta]\nserver = \"https://photos.example.org\"\nkind = \"immich\"\n\n\
    # the daily picture\n[providers.alpha]\nmarket = \"en-US\"\nkind = \"bing\"\n";

#[test]
fn saving_keeps_comments_order_and_layout_of_the_users_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, TEXT).unwrap();
    let mut c = Config::load(&path).unwrap();
    c.transition.duration_ms = 1200;
    c.profile = None;
    c.set_fit("DP-1", wprs_core::Fit::Center);
    c.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    for kept in
        ["# my wallpapers\n", "# slower fades\nduration_ms = 1200 # tuned by hand\n", "# the daily picture\n"]
    {
        assert!(text.contains(kept), "{kept:?} lost in:\n{text}");
    }
    let at = |key: &str| text.find(key).unwrap_or_else(|| panic!("{key} missing in:\n{text}"));
    assert!(at("[providers.zeta]") < at("[providers.alpha]"), "{text}");
    assert!(at("server = ") < at("kind = \"immich\"") && at("market = ") < at("kind = \"bing\""), "{text}");
    assert!(!text.contains("profile") && text.contains("fit = \"center\""), "{text}");
    assert_eq!(Config::load(&path).unwrap(), c);
    c.save(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn a_file_that_does_not_parse_is_replaced_by_a_fresh_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[transition\nduration_ms = 1\n").unwrap();
    let c = Config { profile: Some("01J".into()), ..Config::default() };
    c.save(&path).unwrap();
    assert_eq!(Config::load(&path).unwrap(), c);
}
