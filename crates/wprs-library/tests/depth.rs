use rusqlite::{Connection, params};
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use wprs_library::{Error, Kind, Library};

const NOW: i64 = 1_700_000_000;

fn titles(library: &Library, query: &str) -> Vec<(String, bool)> {
    let found = library.search(query, NOW, 0, 10).unwrap();
    found.into_iter().map(|item| (item.title, item.missing)).collect()
}

fn legacy(db: &Path, path: &Path, source: bool) {
    let sql = "INSERT INTO items(uid, source_id, kind, path, title, size, mtime, qhash, added_at) \
        VALUES (?1, (SELECT id FROM sources WHERE ?2), 'image', ?3, ?1, 1, 0, x'00', 0)";
    let title = path.file_stem().unwrap().to_str().unwrap();
    Connection::open(db).unwrap().execute(sql, params![title, source, path.to_str().unwrap()]).unwrap();
}

#[test]
fn depth_maps_beside_images_are_not_wallpapers() {
    let dir = TempDir::new().unwrap();
    let (db, root) = (dir.path().join("library.db"), dir.path().join("pictures"));
    fs::create_dir_all(&root).unwrap();
    for name in ["hills.jpg", "hills.jpg.depth.png", "dunes.png", "dunes.png.DEPTH.PNG"] {
        fs::write(root.join(name), name).unwrap();
    }
    assert_eq!(Kind::detect(Path::new("dunes.png")), Some(Kind::Image));
    assert_eq!(Kind::detect(&root.join("hills.jpg.depth.png")), None);
    let mut library = Library::open(&db).unwrap();
    let source = library.add_folder(&root, true).unwrap();
    assert_eq!(library.scan(&source, NOW).unwrap().added, 2);
    assert_eq!(titles(&library, "sort:title"), [("dunes".into(), false), ("hills".into(), false)]);
    let map = root.join("hills.jpg.depth.png");
    assert!(matches!(library.import_file(&map, NOW), Err(Error::Invalid("file", _))));

    let loose = dir.path().join("sea.webp.depth.png");
    fs::write(&loose, "sea").unwrap();
    legacy(&db, &map, true);
    legacy(&db, &loose, false);
    assert_eq!(library.scan(&source, NOW).unwrap().missing, 1);
    assert_eq!(library.check_loose(NOW).unwrap().missing, 1);
    let gone = [("hills.jpg.depth".into(), true), ("sea.webp.depth".into(), true)];
    assert_eq!(titles(&library, "missing:yes sort:title"), gone);
    assert_eq!(library.count("missing:no", NOW).unwrap(), 2);
}

#[test]
fn heif_photos_join_only_where_the_platform_decodes_them() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("camera");
    fs::create_dir_all(&root).unwrap();
    for name in ["beach.heic", "boat.HEIF", "pier.jpg"] {
        fs::write(root.join(name), name).unwrap();
    }
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let source = library.add_folder(&root, false).unwrap();
    assert_eq!(library.scan(&source, NOW).unwrap().added, 1);
    assert_eq!(titles(&library, "sort:title"), [("pier".into(), false)]);
    assert!(matches!(library.import_file(root.join("beach.heic"), NOW), Err(Error::Invalid("file", _))));
    wprs_library::platform_decodes_heif();
    assert_eq!(Kind::detect(&root.join("boat.HEIF")), Some(Kind::Image));
    assert_eq!(library.scan(&source, NOW).unwrap().added, 2);
}
