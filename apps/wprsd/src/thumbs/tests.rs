use super::*;

#[test]
fn keys_follow_path_mtime_and_size() {
    let key = key(Path::new("/pictures/a.png"), 1_700_000_000, 42);
    let expected = blake3::hash(b"/pictures/a.png\x001700000000\x0042").to_hex().to_string();
    assert_eq!(key, expected);
    assert_eq!(key.len(), 64);
    assert_ne!(super::key(Path::new("/pictures/a.png"), 1_700_000_001, 42), key);
}

#[test]
fn thumbnails_of_removed_or_changed_items_go_and_failed_decodes_wait() {
    let dir = tempfile::tempdir().unwrap();
    let picture = dir.path().join("a.png");
    fs::write(&picture, b"png").unwrap();
    let mut lib = wprs_library::Library::open_in_memory().unwrap();
    let uid = lib.import_file(&picture, 1).unwrap();
    let item = lib.item(&uid).unwrap();
    let thumbs = Thumbs::new(dir.path());
    let written = || {
        let path = thumbs.path(&item, 256);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"jpg").unwrap();
        path
    };
    let (path, retry) = (written(), Response { retry: true, ..Response::default() });
    assert!(!thumbs.later(&item) && !thumbs.settle(&item, Some(&item), &retry));
    assert!(path.exists() && thumbs.later(&item));
    assert!(thumbs.settle(&item, None, &Response::default()) && !path.exists());
    let (path, changed) = (written(), Item { mtime: item.mtime + 1, ..item.clone() });
    assert!(thumbs.settle(&item, Some(&changed), &Response::default()) && !path.exists());
}

#[test]
fn pruning_deletes_the_oldest_files_first() {
    let dir = tempfile::tempdir().unwrap();
    let thumbs = Thumbs::new(dir.path());
    let small = thumbs.root.join("256");
    fs::create_dir_all(&small).unwrap();
    for (n, age) in [(1, 30), (2, 20), (3, 10)] {
        let path = small.join(format!("{n}.jpg"));
        fs::write(&path, [0; 400]).unwrap();
        let at = SystemTime::now() - Duration::from_secs(age);
        File::options().write(true).open(&path).unwrap().set_modified(at).unwrap();
    }
    thumbs.added(1, 0);
    assert!(!small.join("3.jpg").exists());
    thumbs.prune(1000);
    assert_eq!(thumbs.used.load(Ordering::Relaxed), 0);
    fs::write(small.join("4.jpg"), [0; 400]).unwrap();
    fs::write(small.join("5.jpg"), [0; 400]).unwrap();
    thumbs.prune(1000);
    assert_eq!(thumbs.used.load(Ordering::Relaxed), 800);
    fs::write(small.join("6.jpg"), [0; 400]).unwrap();
    thumbs.prune(1000);
    assert!(thumbs.used.load(Ordering::Relaxed) <= 900);
}
