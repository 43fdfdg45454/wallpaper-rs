use serde::Deserialize;
use std::path::Path;
use wprs_core::{Config, Fit};

#[test]
fn a_section_shows_its_playlist_or_its_image_and_falls_back_as_a_whole() {
    let text = "[default]\nplaylist = \"P1\"\nimage = \"/a.png\"\nfit = \"tile\"\n\
        [outputs.DP-1]\nimage = \"/b.png\"\n[outputs.DP-2]\nfit = \"center\"\n\
        [outputs.HDMI-A-1]\nplaylist = \"P2\"\npreset = \"P3\"\n";
    let c: Config = toml::from_str(text).unwrap();
    let [dp1, dp2, hdmi] = ["DP-1", "DP-2", "HDMI-A-1"].map(|o| c.resolve(o));
    assert_eq!((dp1.image.as_deref(), dp1.playlist, dp1.fit), (Some(Path::new("/b.png")), None, Fit::Tile));
    assert_eq!((dp2.image, dp2.playlist.as_deref(), dp2.fit), (None, Some("P1"), Fit::Center));
    assert_eq!((hdmi.image, hdmi.playlist.as_deref()), (None, Some("P2")));
}

#[test]
fn playlists_and_images_replace_each_other() {
    let mut c = Config::default();
    c.set_image("DP-1", "/b.png".into(), None);
    c.outputs.get_mut("DP-1").unwrap().preset = Some("P3".into());
    c.set_playlist("HDMI-A-1", "P2".into());
    c.set_playlist("", "P1".into());
    assert!(c.outputs.is_empty());
    assert_eq!((c.default.playlist.as_deref(), c.resolve("DP-1").image), (Some("P1"), None));
    c.set_playlist("DP-1", "P2".into());
    c.set_image("DP-1", "/b.png".into(), Some(Fit::Fit));
    let dp1 = &c.outputs["DP-1"];
    assert_eq!((dp1.playlist.as_deref(), dp1.preset.as_deref(), dp1.fit), (None, None, Some(Fit::Fit)));
    c.set_image("", "/a.png".into(), None);
    assert_eq!(
        (c.default.playlist.as_deref(), c.resolve("DP-1").image.as_deref()),
        (None, Some(Path::new("/a.png")))
    );
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Provider {
    Bing { market: String },
    Immich { server: String },
}

#[test]
fn unreadable_providers_are_reported_and_kept_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let photos = "[providers.photos]\nkind = \"immich\"\npassword = \"hand-written\"\nserver = \"https://photos.example.org\"\n";
    let text = format!(
        "profile = \"01J\"\n[library]\nwatch = false\n[providers.daily]\nkind = \"bing\"\nmarket = \"en-US\"\n{photos}[providers.later]\nkind = \"future\"\n"
    );
    std::fs::write(&path, text).unwrap();
    let c = Config::load(&path).unwrap();
    assert_eq!((c.profile.as_deref(), c.library.watch, c.repos.auto_refresh), (Some("01J"), false, true));
    let mut read = c.read_providers::<Provider>();
    assert_eq!(read.next(), Some(("daily", Ok(Provider::Bing { market: "en-US".into() }))));
    let errors: Vec<_> = read.map(|(id, r)| (id, r.unwrap_err())).collect();
    assert!(errors[0].0 == "later" && errors[0].1.contains("unknown variant `future`"), "{errors:?}");
    assert!(errors[1].0 == "photos" && errors[1].1.contains("unknown field `password`"), "{errors:?}");
    assert!(errors.iter().all(|(_, e)| !e.contains("hand-written")));
    c.save(&path).unwrap();
    assert!(std::fs::read_to_string(&path).unwrap().contains(photos));
    assert_eq!(Config::load(&path).unwrap(), c);
}

#[test]
fn parse_errors_name_the_position_but_never_quote_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        "[default]\nfit = \"fill\"\n[providers.x]\nkind = \"webdav\"\npassword = \"hand-written\n",
    )
    .unwrap();
    let error = Config::load(&path).unwrap_err().to_string();
    assert!(error.starts_with("invalid configuration at line 5, column 25: "), "{error}");
    assert!(!error.contains("hand-written") && !error.contains('\n'), "{error}");
    std::fs::write(&path, "[default]\nfti = \"fill\"\n").unwrap();
    let error = Config::load(&path).unwrap_err().to_string();
    assert!(error.starts_with("invalid configuration at line 2, column 1: unknown field `fti`"), "{error}");
}

#[test]
fn library_and_repository_options() {
    let mut c = Config::default();
    c.set_option("library.thumb_cache_mb", "1024").unwrap();
    c.set_option("repos.auto_refresh", "false").unwrap();
    assert!(c.set_option("library.thumb_cache_mb", "10").is_err());
    assert_eq!((c.library.watch, c.library.thumb_cache_mb, c.repos.auto_refresh), (true, 1024, false));
    let c: Config = toml::from_str("[library]\nthumb_cache_mb = 1").unwrap();
    assert_eq!(c.clone().normalize(), ["library.thumb_cache_mb"]);
}
