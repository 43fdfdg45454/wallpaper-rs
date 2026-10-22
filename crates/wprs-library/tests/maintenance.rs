use rusqlite::Connection;
use serde_json::json;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::ops::ControlFlow;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use wprs_format::{Limits, Manifest, SCHEMA};
use wprs_library::{Error, Library, Meta, Rotation, Rules, SCHEMA_VERSION, ScanReport};

const NOW: i64 = 1_700_000_000;

fn package(dir: &Path, version: &str, tags: &[&str], pixels: &str) -> PathBuf {
    let manifest = Manifest {
        schema: SCHEMA.into(),
        id: "org.example.calm".into(),
        version: version.into(),
        title: "Calm Water".into(),
        description: String::new(),
        authors: Vec::new(),
        license: "CC0-1.0".into(),
        kind: wprs_format::Kind::Image,
        entry: "image.png".into(),
        preview: None,
        tags: tags.iter().map(ToString::to_string).collect(),
        properties: BTreeMap::new(),
    };
    let source = dir.join(format!("source-{version}"));
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    fs::write(source.join("image.png"), pixels).unwrap();
    let target = dir.join(format!("calm-{version}.wpak"));
    wprs_format::build(&source, &target, &Limits::default()).unwrap();
    target
}

#[test]
fn the_reader_sees_the_writer_but_cannot_write_or_open_another_schema() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("data/library.db");
    let mut writer = Library::open(&path).unwrap();
    let mut reader = Library::open_read_only(&path).unwrap();
    let uid = writer.create_playlist("Evening", None, &Rules::default()).unwrap();
    assert_eq!(reader.playlist("evening").unwrap().uid, uid);
    assert!(matches!(reader.create_playlist("Morning", None, &Rules::default()), Err(Error::Sqlite(_))));
    let raw = Connection::open(&path).unwrap();
    raw.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
    assert!(matches!(Library::open_read_only(&path), Err(Error::Newer(..))));
    raw.pragma_update(None, "user_version", SCHEMA_VERSION - 1).unwrap();
    assert!(matches!(Library::open_read_only(&path), Err(Error::Invalid("database", _))));
    assert!(Library::open_read_only(dir.path().join("none.db")).is_err());
}

#[test]
fn opening_a_first_version_database_gives_rotations_a_current_item() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("library.db");
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch(include_str!("../src/schema.sql")).unwrap();
    let rows = "PRAGMA user_version = 1; \
        INSERT INTO playlists(uid, name) VALUES ('01ARZ3NDEKTSV4RRFFQ69G5FAV', 'Evening'); \
        INSERT INTO rotation(output, playlist_id, state) VALUES ('DP-1', 1, '{\"seed\": 1, \"cycle\": 0, \"position\": 0}');";
    raw.execute_batch(rows).unwrap();
    let mut library = Library::open(&path).unwrap();
    let version: u32 = raw.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    let rotation = library.rotation("DP-1").unwrap().unwrap();
    assert_eq!((version, rotation.current.as_deref()), (SCHEMA_VERSION, None));
    let current = Some("01BX5ZZKBKACTAV9WEVGEMMVRZ".to_string());
    let synced = Rotation { output: format!("@{}", rotation.playlist), current, ..rotation };
    library.save_rotation(&synced).unwrap();
    assert_eq!(library.rotation(&synced.output).unwrap(), Some(synced));
}

#[test]
fn scans_commit_in_batches_report_progress_and_stop_when_cancelled() {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let root = dir.path().join("pictures");
    fs::create_dir_all(&root).unwrap();
    (0..300).for_each(|n| fs::write(root.join(format!("b{n:03}.png")), n.to_string()).unwrap());
    fs::write(root.join(OsStr::from_bytes(b"not utf-8 \xff.png")), "skipped").unwrap();
    let source = library.add_folder(&root, true).unwrap();
    let mut calls = Vec::new();
    let mut record = |done, total| {
        calls.push((done, total));
        ControlFlow::Continue(())
    };
    assert_eq!(library.scan_with(&source, NOW, &mut record).unwrap().added, 300);
    assert_eq!(calls, [(256, 300), (300, 300)]);
    fs::remove_file(root.join("b000.png")).unwrap();
    (0..100).for_each(|n| fs::write(root.join(format!("a{n:03}.png")), format!("a{n}")).unwrap());
    let cancelled = library.scan_with(&source, NOW, &mut |_, _| ControlFlow::Break(()));
    assert!(matches!(cancelled, Err(Error::Cancelled)));
    assert_eq!((library.count("", NOW).unwrap(), library.count("missing:", NOW).unwrap()), (400, 0));
    let report = library.scan(&source, NOW).unwrap();
    assert_eq!(report, ScanReport { missing: 1, ..ScanReport::default() });
}

#[test]
fn loose_items_become_missing_when_their_file_goes_and_return_with_it() {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let root = dir.path().join("pictures");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("kept.png"), "kept").unwrap();
    let source = library.add_folder(&root, true).unwrap();
    library.scan(&source, NOW).unwrap();
    fs::remove_file(root.join("kept.png")).unwrap();
    let loose = dir.path().join("loose.png");
    fs::write(&loose, "one").unwrap();
    let uid = library.import_file(&loose, NOW).unwrap();
    library.set_meta(&uid, &Meta { width: Some(1), height: Some(1), ..Meta::default() }).unwrap();
    fs::remove_file(&loose).unwrap();
    assert_eq!(library.check_loose(NOW).unwrap(), ScanReport { missing: 1, ..ScanReport::default() });
    assert!(library.item(&uid).unwrap().missing);
    assert_eq!(library.check_loose(NOW).unwrap(), ScanReport::default());
    fs::write(&loose, "changed").unwrap();
    assert_eq!(library.check_loose(NOW).unwrap(), ScanReport { updated: 1, ..ScanReport::default() });
    let item = library.item(&uid).unwrap();
    assert_eq!((item.missing, item.meta.width, item.size), (false, None, 7));
    assert_eq!(library.count("missing:", NOW).unwrap(), 0);
}

#[test]
fn relinking_keeps_the_item_and_its_user_data_and_merges_package_tags() {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let old = package(dir.path(), "1.2.0", &["water"], "old pixels");
    let uid = library.import_file(&old, NOW).unwrap();
    library.set_title(&uid, "My water").unwrap();
    library.retag(&[&uid], &["mine"], &["water"]).unwrap();
    library.set_rating(&uid, 4).unwrap();
    library.set_favorite(&uid, true).unwrap();
    library.set_meta(&uid, &Meta { width: Some(64), height: Some(64), ..Meta::default() }).unwrap();
    let list = library.create_playlist("Calm", None, &Rules::default()).unwrap();
    library.add_to_playlist(&list, &[&uid]).unwrap();
    library.create_preset(Some(&uid), "Dim", &json!({"fit": "fill"})).unwrap();
    library.mark_used(&uid, "DP-1", NOW).unwrap();
    let new = package(dir.path(), "1.3.0", &["water", "night"], "new and larger pixels");
    library.relink_item(&uid, &new, NOW + 60).unwrap();
    let item = library.item(&uid).unwrap();
    assert_eq!(
        (&item.path, item.title.as_str(), item.rating, item.favorite, item.size),
        (&fs::canonicalize(&new).unwrap(), "My water", 4, true, fs::metadata(&new).unwrap().len())
    );
    assert_eq!(
        (item.package.unwrap().version, item.tags, item.meta),
        ("1.3.0".into(), vec!["mine".into(), "night".into(), "water".into()], Meta::default())
    );
    assert_eq!(library.playlist_items(&list, NOW).unwrap()[0].uid, uid);
    assert_eq!(library.presets(Some(&uid)).unwrap().len(), 1);
    assert_eq!(library.history("DP-1", 10).unwrap()[0].item.as_deref(), Some(uid.as_str()));
    library.import_file(&old, NOW).unwrap();
    assert!(matches!(library.relink_item(&uid, &old, NOW), Err(Error::Exists("item", _))));
    assert!(matches!(library.relink_item("NOPE", &new, NOW), Err(Error::NotFound("item", _))));
}

#[test]
fn unanalyzed_items_are_present_ones_without_metadata_oldest_first() {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let mut uids = Vec::new();
    for (name, age) in ["d", "c", "b", "a", "e", "f"].into_iter().zip(0..) {
        let path = dir.path().join(format!("{name}.png"));
        fs::write(&path, name).unwrap();
        uids.push(library.import_file(&path, NOW + age).unwrap());
    }
    library.set_meta(&uids[1], &Meta { width: Some(1), height: Some(1), ..Meta::default() }).unwrap();
    library.set_meta(&uids[2], &Meta { error: Some("unreadable".into()), ..Meta::default() }).unwrap();
    library.set_meta(&uids[3], &Meta { duration_ms: Some(1000), ..Meta::default() }).unwrap();
    fs::remove_file(dir.path().join("e.png")).unwrap();
    library.check_loose(NOW).unwrap();
    let titles = |limit| library.unanalyzed(limit).unwrap().into_iter().map(|i| i.title).collect::<Vec<_>>();
    assert_eq!((titles(10), titles(1)), (vec!["d".to_string(), "f".into()], vec!["d".into()]));
}
