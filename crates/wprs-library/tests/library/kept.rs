use super::*;

#[test]
fn rotation_state_survives_reopening_the_library() {
    let dir = TempDir::new().unwrap();
    let (mut library, _, [a, b, c]) = folder_with_three_items(&dir);
    let rules = Rules { order: Order::Shuffle, ..Rules::default() };
    let list = library.create_playlist("All", None, &rules).unwrap();
    library.add_to_playlist(&list, &[&a.uid, &b.uid, &c.uid]).unwrap();
    let playlist = resolve(&library, &list, false);
    let mut state = PlaylistState::new(7);
    state.advance(&playlist, DateTime::from_timestamp(NOW, 0).unwrap());
    let rotation = Rotation {
        output: "DP-1".into(),
        playlist: list.clone(),
        state: state.clone(),
        next_at: Some(NOW + 600),
        current: Some(b.uid.clone()),
    };
    library.save_rotation(&rotation).unwrap();
    drop(library);
    let mut library = open(&dir);
    let loaded = library.rotation("DP-1").unwrap().unwrap();
    assert_eq!(loaded, rotation);
    assert_eq!(loaded.state.current(&playlist), state.current(&playlist));
    library.save_rotation(&Rotation { next_at: None, ..rotation }).unwrap();
    assert_eq!(library.rotations().unwrap().len(), 1);
    assert_eq!(library.rotation("HDMI-A-1").unwrap(), None);
    library.remove_playlist(&list).unwrap();
    assert!(library.rotations().unwrap().is_empty());
}

#[test]
fn presets_and_profiles_reject_duplicate_names() {
    let dir = TempDir::new().unwrap();
    let (mut library, _, [a, ..]) = folder_with_three_items(&dir);
    let calm = library.create_preset(None, "Calm", &json!({"fit": "cover"})).unwrap();
    let error = library.create_preset(None, "calm", &json!({})).unwrap_err();
    assert_eq!(error.to_string(), "preset \"calm\" already exists");
    library.create_preset(Some(&a.uid), "Calm", &json!({"volume": 0.5})).unwrap();
    assert!(matches!(
        library.create_preset(Some(&a.uid), "CALM", &json!({})),
        Err(Error::Exists("preset", _))
    ));
    library.update_preset(&calm, &json!({"fit": "contain"})).unwrap();
    assert_eq!(library.preset(&calm).unwrap().value, json!({"fit": "contain"}));
    assert!(matches!(library.preset("01J0000000000000000000NONE"), Err(Error::NotFound("preset", _))));
    assert_eq!(library.presets(None).unwrap()[0].value, json!({"fit": "contain"}));
    assert_eq!(library.presets(Some(&a.uid)).unwrap()[0].item.as_deref(), Some(a.uid.as_str()));
    library.remove_preset(&calm).unwrap();
    assert!(library.presets(None).unwrap().is_empty());
    let settings = |package: &str| json!({"package": package, "values": {"speed": 2}});
    let night = library.create_preset(None, "Night", &settings("org.example.a")).unwrap();
    library.create_preset(None, "night", &settings("org.example.b")).unwrap();
    assert!(matches!(
        library.create_preset(None, "NIGHT", &settings("org.example.a")),
        Err(Error::Exists(..))
    ));
    library.create_preset(None, "Dusk", &settings("org.example.a")).unwrap();
    assert!(matches!(library.rename_preset(&night, "dusk"), Err(Error::Exists(..))));
    library.rename_preset(&night, "Late").unwrap();
    assert_eq!(library.preset(&night).unwrap().name, "Late");
    assert!(matches!(library.rename_preset("01J0000000000000000000NONE", "x"), Err(Error::NotFound(..))));

    let empty = json!({});
    let outputs = json!({"DP-1": {"playlist": "Morning"}});
    library.create_profile("Work", &outputs, &empty, &json!({"on_battery": false}), 1).unwrap();
    let home = library.create_profile("Home", &json!({"": {"color": "#000000"}}), &empty, &empty, 5).unwrap();
    let error = library.create_profile("work", &empty, &empty, &empty, 0).unwrap_err();
    assert_eq!(error.to_string(), "profile \"work\" already exists");
    let mut profile = library.profile("Work").unwrap();
    assert_eq!(profile.outputs, outputs);
    profile.name = "Home".into();
    assert!(matches!(library.update_profile(&profile), Err(Error::Exists("profile", _))));
    (profile.name, profile.priority) = ("Office".into(), 9);
    library.update_profile(&profile).unwrap();
    let names: Vec<String> = library.profiles().unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["Office", "Home"]);
    library.remove_profile(&home).unwrap();
    assert!(matches!(library.profile(&home), Err(Error::NotFound("profile", _))));
}
