use super::turns::{SETTLE, settle};
use super::*;
use std::time::Duration;
use wprs_core::model::OutputPlaylist;

#[test]
fn library_changes_during_scans_resolve_at_most_every_few_seconds() {
    assert!(!settle(false, Duration::ZERO));
    assert!(settle(true, Duration::from_secs(1)));
    assert!(!settle(true, SETTLE));
}

#[test]
fn stopping_keeps_the_item_shown() {
    let shown = |uid: &str, path: &str| {
        let playlist =
            OutputPlaylist { uid: uid.into(), name: uid.into(), position: 0, count: 1, next_at: None };
        Playing { playlist, item: None, path: Some(path.into()), error: None, transition: None }
    };
    let mut config = Config::default();
    config.set_playlist("", "A".into());
    config.set_playlist("DP-2", "B".into());
    let playing =
        HashMap::from([("DP-1".into(), shown("A", "/w/a.png")), ("DP-2".into(), shown("B", "/w/b.png"))]);
    let mut one = config.clone();
    assert_eq!(stop_sections(&mut one, &playing, "DP-2", None), ["B"]);
    assert_eq!(
        (one.resolve("DP-2").image, one.resolve("DP-1").playlist),
        (Some("/w/b.png".into()), Some("A".into()))
    );
    assert_eq!(stop_sections(&mut config, &playing, "", Some("A")), ["A"]);
    assert_eq!((&config.default.image, &config.default.playlist), (&Some("/w/a.png".into()), &None));
    assert_eq!(config.resolve("DP-2").playlist.as_deref(), Some("B"));
}
