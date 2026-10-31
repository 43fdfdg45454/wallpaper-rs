use super::*;
use std::fs;
use std::mem::MaybeUninit;

fn drain(folders: &Folders) -> Vec<(Option<String>, Option<String>)> {
    let fd = folders.fd.as_ref().unwrap();
    rustix::io::ioctl_fionbio(fd, true).unwrap();
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut reader = inotify::Reader::new(fd, &mut buffer);
    let mut seen = Vec::new();
    while let Ok(event) = reader.next() {
        let name = event.file_name().map(|n| OsStr::from_bytes(n.to_bytes()));
        let source = folders.handle(event.wd(), event.events(), name);
        seen.push((name.map(|n| n.to_string_lossy().into_owned()), source));
    }
    seen
}

fn watched(folders: &Folders) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = lock(&folders.watches).values().map(|w| w.dir.clone()).collect();
    dirs.sort();
    dirs
}

#[test]
fn links_moved_and_removed_folders_keep_the_watches_right() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("pictures");
    fs::create_dir_all(root.join("trip")).unwrap();
    let folders = Folders::new();
    folders.add(&mut lock(&folders.watches), "S", &root, true);
    assert_eq!(watched(&folders), [root.clone(), root.join("trip")]);
    fs::write(dir.path().join("elsewhere.png"), b"png").unwrap();
    fs::hard_link(dir.path().join("elsewhere.png"), root.join("linked.png")).unwrap();
    let writing = fs::File::create(root.join("new.png")).unwrap();
    let seen = drain(&folders);
    let source = |file: &str| seen.iter().find(|(name, _)| name.as_deref() == Some(file)).unwrap().1.clone();
    assert_eq!((source("linked.png"), source("new.png")), (Some("S".into()), None));
    drop(writing);
    let closed = (Some("new.png".into()), Some("S".into()));
    let started = std::time::Instant::now();
    while !drain(&folders).contains(&closed) {
        assert!(started.elapsed().as_secs() < 5, "the written file was never reported");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    fs::rename(root.join("trip"), dir.path().join("away")).unwrap();
    fs::create_dir(root.join("later")).unwrap();
    drain(&folders);
    assert_eq!(watched(&folders), [root.clone(), root.join("later")]);
    fs::remove_dir(root.join("later")).unwrap();
    fs::write(dir.path().join("away").join("gone.png"), b"png").unwrap();
    assert!(drain(&folders).iter().all(|(name, _)| name.as_deref() != Some("gone.png")));
    assert_eq!(watched(&folders), std::slice::from_ref(&root));
    fs::rename(&root, dir.path().join("moved")).unwrap();
    assert!(drain(&folders).iter().any(|(_, source)| source.as_deref() == Some("S")));
    assert!(watched(&folders).is_empty());
}
