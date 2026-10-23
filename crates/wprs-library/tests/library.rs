use chrono::DateTime;
use rusqlite::Connection;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use wprs_format::{Limits, Manifest, SCHEMA_V1};
use wprs_library::{Error, Item, Kind, Library, Meta, PackageInfo, Rotation, Rules, SCHEMA_VERSION, Used};
use wprs_schedule::{Order, Playlist, PlaylistState};

const NOW: i64 = 1_700_000_000;

fn write(path: &Path, bytes: &[u8]) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    fs::canonicalize(path).unwrap()
}

fn open(dir: &TempDir) -> Library {
    Library::open(dir.path().join("data/library.db")).unwrap()
}

fn find(library: &Library, query: &str) -> Item {
    let mut found = library.search(query, NOW, 0, 10).unwrap();
    assert_eq!(found.len(), 1, "{query}: {found:?}");
    found.remove(0)
}

fn resolve(library: &Library, key: &str, on_battery: bool) -> Playlist {
    library.resolve(key, NOW, on_battery, &HashSet::new()).unwrap().playlist
}

fn mode(path: impl AsRef<Path>) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn counts(library: &mut Library, source: &str, now: i64) -> [u32; 4] {
    let report = library.scan(source, now).unwrap();
    [report.added, report.updated, report.moved, report.missing]
}

fn folder_with_three_items(dir: &TempDir) -> (Library, String, [Item; 3]) {
    let mut library = open(dir);
    let root = dir.path().join("pictures");
    ["a.jpg", "b.png", "c.mp4"].iter().for_each(|name| drop(write(&root.join(name), name.as_bytes())));
    let source = library.add_folder(&root, true).unwrap();
    library.scan(&source, NOW).unwrap();
    let items = library.search("sort:title", NOW, 0, 10).unwrap().try_into().unwrap();
    (library, source, items)
}

#[test]
fn migrations_run_on_an_empty_database_and_reopening_keeps_data() {
    let dir = TempDir::new().unwrap();
    let uid = open(&dir).create_playlist("Evening", None, &Rules::default()).unwrap();
    let raw = Connection::open(dir.path().join("data/library.db")).unwrap();
    let version: u32 = raw.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    let journal: String = raw.pragma_query_value(None, "journal_mode", |r| r.get(0)).unwrap();
    let fts5: bool =
        raw.query_row("SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |r| r.get(0)).unwrap();
    assert_eq!((version, journal.as_str(), fts5), (SCHEMA_VERSION, "wal", true));
    let reopened = open(&dir);
    assert_eq!(reopened.playlist(&uid).unwrap().name, "Evening");
    assert_eq!(reopened.playlist("evening").unwrap().uid, uid);
}

#[test]
fn a_database_from_a_newer_build_is_rejected() {
    let dir = TempDir::new().unwrap();
    drop(open(&dir));
    let raw = Connection::open(dir.path().join("data/library.db")).unwrap();
    raw.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
    let error = Library::open(dir.path().join("data/library.db")).unwrap_err();
    assert!(matches!(error, Error::Newer(version, _) if version == SCHEMA_VERSION + 1), "{error}");
}

#[test]
fn the_database_files_and_their_folder_are_private() {
    let dir = TempDir::new().unwrap();
    let library = open(&dir);
    let data = dir.path().join("data");
    assert_eq!(mode(&data), 0o700);
    for name in ["library.db", "library.db-wal", "library.db-shm"] {
        assert_eq!(mode(data.join(name)), 0o600, "{name}");
    }
    drop(library);
}

#[test]
fn scanning_refinds_moved_files_and_marks_vanished_ones_missing() {
    let dir = TempDir::new().unwrap();
    let mut library = open(&dir);
    let root = dir.path().join("pictures");
    write(&root.join("waves.jpg"), b"waves");
    write(&root.join("dunes.PNG"), b"dunes");
    write(&root.join("clips/rain.webm"), b"rain");
    write(&root.join("notes.txt"), b"skipped");
    write(&root.join(".hidden.jpg"), b"skipped");
    let source = library.add_folder(&root, true).unwrap();
    assert!(matches!(library.add_folder(&root, false), Err(Error::Exists("source", _))));
    assert_eq!(counts(&mut library, &source, NOW), [3, 0, 0, 0]);
    let waves = find(&library, "waves");
    assert_eq!(
        (waves.kind, waves.source.as_deref(), waves.added_at),
        (Kind::Image, Some(source.as_str()), NOW)
    );
    assert_eq!(find(&library, "rain").kind, Kind::Video);
    library.add_tag(&waves.uid, "sea").unwrap();

    fs::create_dir_all(root.join("moved")).unwrap();
    fs::rename(root.join("waves.jpg"), root.join("moved/renamed.jpg")).unwrap();
    fs::remove_file(root.join("dunes.PNG")).unwrap();
    write(&root.join("clips/rain.webm"), b"heavier rain");
    assert_eq!(counts(&mut library, &source, NOW + 60), [0, 1, 1, 1]);
    let moved = library.item(&waves.uid).unwrap();
    assert_eq!(
        (moved.path, moved.tags, moved.added_at),
        (fs::canonicalize(root.join("moved/renamed.jpg")).unwrap(), vec!["sea".to_string()], NOW)
    );
    assert!(find(&library, "dunes").missing);
    assert_eq!(find(&library, "renamed").uid, waves.uid);

    write(&root.join("dunes.PNG"), b"dunes");
    assert_eq!(counts(&mut library, &source, NOW + 120), [0, 1, 0, 0]);
    assert!(!find(&library, "dunes").missing);
    assert_eq!(library.sources().unwrap()[0].scanned_at, Some(NOW + 120));
}

#[test]
fn shallow_sources_skip_subfolders_and_loose_files_have_no_source() {
    let dir = TempDir::new().unwrap();
    let mut library = open(&dir);
    let root = dir.path().join("pictures");
    write(&root.join("top.jpg"), b"top");
    write(&root.join("deep/below.jpg"), b"below");
    let source = library.add_folder(&root, false).unwrap();
    assert_eq!(counts(&mut library, &source, NOW), [1, 0, 0, 0]);
    let loose = write(&dir.path().join("loose.mkv"), b"loose");
    let uid = library.import_file(&loose, NOW).unwrap();
    assert_eq!(library.import_file(&loose, NOW).unwrap(), uid);
    let item = library.item(&uid).unwrap();
    assert_eq!((item.kind, item.source, item.path), (Kind::Video, None, loose));
    let text = write(&dir.path().join("readme.txt"), b"text");
    assert!(matches!(library.import_file(&text, NOW), Err(Error::Invalid("file", _))));

    fs::remove_dir_all(&root).unwrap();
    assert!(matches!(library.scan(&source, NOW), Err(Error::Io(_))));
    assert!(library.sources().unwrap()[0].error.is_some());
    assert!(!find(&library, "top").missing);
    library.remove_source(&source).unwrap();
    assert!(library.sources().unwrap().is_empty());
    assert_eq!(library.count("", NOW).unwrap(), 1);
    let raw = Connection::open(dir.path().join("data/library.db")).unwrap();
    let indexed: u32 = raw
        .query_row("SELECT count(*) FROM items_fts WHERE items_fts MATCH 'top'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(indexed, 0);
}

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
        facets: vec!["android".into()],
        confirmed: Vec::new(),
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
