use super::*;

#[test]
fn packages_are_described_by_their_manifest() {
    let dir = TempDir::new().unwrap();
    let mut library = open(&dir);
    let manifest = Manifest {
        schema: SCHEMA_V1.into(),
        id: "org.example.calm".into(),
        version: "1.2.0".into(),
        title: "Calm Water".into(),
        license: "CC0-1.0".into(),
        kind: wprs_format::Kind::Image,
        entry: "image.png".into(),
        tags: vec!["water".into()],
        ..Default::default()
    };
    let package = dir.path().join("package");
    write(&package.join("manifest.json"), manifest.to_json().unwrap().as_bytes());
    write(&package.join("image.png"), b"pixels");
    let root = dir.path().join("pictures");
    fs::create_dir_all(&root).unwrap();
    wprs_format::build(&package, root.join("calm.wpak"), &Limits::default()).unwrap();
    write(&root.join("broken.wpak"), b"not a zip archive");
    let source = library.add_folder(&root, true).unwrap();
    assert_eq!(counts(&mut library, &source, NOW), [2, 0, 0, 0]);
    let calm = find(&library, "kind:package tag:water");
    assert_eq!(
        (library.count("kind:scene", NOW).unwrap(), library.count("-kind:scene", NOW).unwrap()),
        (0, 2)
    );
    let info = PackageInfo {
        id: "org.example.calm".into(),
        version: "1.2.0".into(),
        kind: "image".into(),
        license: "CC0-1.0".into(),
        content: None,
        photosensitive: None,
        facets: vec!["android".into(), "desktop".into()],
        confirmed: Vec::new(),
        base: None,
    };
    assert_eq!((calm.title.as_str(), calm.package), ("Calm Water", Some(info)));
    let broken = find(&library, "broken");
    assert!(broken.package.is_none() && broken.meta.error.is_some());
}

#[test]
fn items_keep_titles_tags_ratings_favorites_and_metadata() {
    let dir = TempDir::new().unwrap();
    let (mut library, _, [a, b, _]) = folder_with_three_items(&dir);
    library.set_title(&a.uid, "  First light ").unwrap();
    for tag in ["Sea", "sea", "night"] {
        library.add_tag(&a.uid, tag).unwrap();
    }
    library.add_tag(&b.uid, "SEA").unwrap();
    assert_eq!(library.tags().unwrap(), [("night".to_string(), 1), ("Sea".to_string(), 2)]);
    library.remove_tag(&a.uid, "NIGHT").unwrap();
    library.set_rating(&a.uid, 5).unwrap();
    assert!(matches!(library.set_rating(&a.uid, 6), Err(Error::Invalid("rating", _))));
    assert!(matches!(library.set_title(&a.uid, " "), Err(Error::Invalid("title", _))));
    library.set_favorite(&a.uid, true).unwrap();
    let meta = Meta { width: Some(3840), height: Some(2160), color: Some(0x3366ff), ..Meta::default() };
    library.set_meta(&a.uid, &meta).unwrap();
    let item = library.item(&a.uid).unwrap();
    assert_eq!(
        (item.title.as_str(), item.tags, item.rating, item.favorite),
        ("First light", vec!["Sea".into()], 5, true)
    );
    assert_eq!(item.meta, meta);
    library.remove_item(&b.uid).unwrap();
    assert!(matches!(library.item(&b.uid), Err(Error::NotFound("item", _))));
    assert_eq!(library.tags().unwrap(), [("Sea".to_string(), 1)]);
}

#[test]
fn use_updates_counters_and_history_is_kept_per_output_and_trimmed() {
    let dir = TempDir::new().unwrap();
    let (mut library, _, [a, ..]) = folder_with_three_items(&dir);
    for second in 0..1005 {
        library.mark_used(&a.uid, "DP-1", NOW + second).unwrap();
    }
    library.mark_used(&a.uid, "HDMI-A-1", NOW + 2000).unwrap();
    let item = library.item(&a.uid).unwrap();
    assert_eq!((item.used_at, item.use_count), (Some(NOW + 2000), 1006));
    let history = library.history("DP-1", 5000).unwrap();
    assert_eq!((history.len(), &history[0]), (999, &Used { at: NOW + 1004, item: Some(a.uid.clone()) }));
    library.remove_item(&a.uid).unwrap();
    assert_eq!(library.history("HDMI-A-1", 10).unwrap(), [Used { at: NOW + 2000, item: None }]);
}
