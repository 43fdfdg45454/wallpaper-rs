use super::*;

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
