use super::*;

#[test]
fn manual_playlists_keep_their_order_and_resolve_to_schedule_playlists() {
    let dir = TempDir::new().unwrap();
    let (mut library, _, [a, b, c]) = folder_with_three_items(&dir);
    let rules =
        Rules { order: Order::Shuffle, interval: Duration::from_secs(90), synced: true, ..Rules::default() };
    let list = library.create_playlist("Morning", None, &rules).unwrap();
    library.add_to_playlist(&list, &[&a.uid, &b.uid, &c.uid]).unwrap();
    library.move_in_playlist("Morning", 2, 0).unwrap();
    library.remove_from_playlist(&list, &a.uid).unwrap();
    assert!(matches!(library.move_in_playlist(&list, 0, 5), Err(Error::Invalid("position", _))));
    let titles: Vec<String> =
        library.playlist_items(&list, NOW).unwrap().into_iter().map(|i| i.title).collect();
    assert_eq!(titles, ["c", "b"]);
    let expected = Playlist {
        id: list.clone(),
        items: vec![c.path.clone(), b.path.clone()],
        order: Order::Shuffle,
        interval: Duration::from_secs(90),
        per_output: false,
    };
    assert_eq!(resolve(&library, &list, true), expected);
    let mut info = library.playlist(&list).unwrap();
    info.rules.skip_videos_on_battery = true;
    library.update_playlist(&info).unwrap();
    assert_eq!(resolve(&library, &list, true).items, [b.path.as_path()]);
    assert_eq!(resolve(&library, &list, false).items.len(), 2);
    let error = library.create_playlist(" morning ", None, &Rules::default()).unwrap_err();
    assert_eq!(error.to_string(), "playlist \"morning\" already exists");
}

#[test]
fn smart_playlists_follow_their_query_and_skip_missing_items() {
    let dir = TempDir::new().unwrap();
    let (mut library, source, [a, b, c]) = folder_with_three_items(&dir);
    library.add_tag(&a.uid, "sea").unwrap();
    let smart = library.create_playlist("Sea", Some("tag:sea sort:title"), &Rules::default()).unwrap();
    assert_eq!(resolve(&library, &smart, false).items, [a.path.as_path()]);
    library.add_tag(&c.uid, "sea").unwrap();
    assert_eq!(resolve(&library, "sea", false).items, [a.path.clone(), c.path.clone()]);
    fs::remove_file(&a.path).unwrap();
    library.scan(&source, NOW).unwrap();
    assert_eq!(resolve(&library, &smart, false).items, [c.path.as_path()]);
    assert!(matches!(library.add_to_playlist(&smart, &[&b.uid]), Err(Error::Invalid("playlist", _))));
    let bad = library.create_playlist("Bad", Some("nope:1"), &Rules::default());
    assert!(matches!(bad, Err(Error::Query(_))));
    library.remove_playlist("Sea").unwrap();
    assert!(library.playlists().unwrap().is_empty());
}

#[test]
fn rules_use_defaults_and_reject_unknown_fields_and_bad_values() {
    let value =
        json!({"order": "shuffle", "interval": "1h30m", "at": ["07:30", "18:00"], "days": ["Mon", "Sat"]});
    let rules: Rules = serde_json::from_value(value).unwrap();
    assert_eq!(
        (rules.order, rules.interval, rules.resume, rules.synced),
        (Order::Shuffle, Duration::from_secs(5400), true, false)
    );
    let text = serde_json::to_value(&rules).unwrap();
    assert_eq!((&text["at"], &text["interval"]), (&json!(["07:30", "18:00"]), &json!("1h30m")));
    assert_eq!(serde_json::from_value::<Rules>(text).unwrap(), rules);
    let bad = [
        json!({"bogus": true}),
        json!({"at": ["25:00"]}),
        json!({"interval": "soon"}),
        json!({"days": ["Someday"]}),
    ];
    for value in bad {
        assert!(serde_json::from_value::<Rules>(value.clone()).is_err(), "{value}");
    }
}
