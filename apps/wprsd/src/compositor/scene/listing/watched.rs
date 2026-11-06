use super::tests::{listed, state};
use super::*;
use std::sync::atomic::AtomicUsize;

fn watched(dir: &std::path::Path) -> Option<(SceneState, Arc<AtomicUsize>, Instant)> {
    let mut state = state(Some(dir));
    let woken = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&woken);
    state.waker(|| {
        Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        })
    });
    let start = Instant::now();
    assert!(listed(&mut state, start), "the first listing never arrived");
    if state.listed.watch.is_none() {
        eprintln!("no change notices on this system: the folder is looked at every 2 s instead");
        return None;
    }
    Some((state, woken, start + Duration::from_secs(600)))
}

fn told(state: &SceneState) -> bool {
    let changed = || state.listed.watch.as_ref().is_some_and(|w| w.changed.load(Ordering::Acquire));
    (0..2000).any(|_| changed() || (std::thread::sleep(Duration::from_millis(5)), false).1)
}

fn picture(dir: &std::path::Path, name: &str) {
    image::RgbaImage::new(2, 2).save(dir.join(name)).unwrap();
}

#[test]
fn a_change_heard_while_a_look_runs_starts_another_look_after_it() {
    let dir = tempfile::tempdir().unwrap();
    picture(dir.path(), "a.png");
    let Some((mut state, _, later)) = watched(dir.path()) else { return };
    let (tx, rx) = mpsc::channel();
    state.listed.scanning = Some(rx);
    picture(dir.path(), "c.png");
    assert!(told(&state), "no change notice arrived");
    assert!(!state.listing(later), "nothing was listed yet");
    assert!(told(&state), "the notice was dropped while the look ran");
    tx.send((state.listed.text.clone(), state.listed.files.clone(), None)).unwrap();
    assert!(listed(&mut state, later) && state.listed.text.contains("c.png"), "the change was never listed");
}

#[test]
fn a_hidden_scene_drops_its_watch_and_looks_again_when_shown() {
    let dir = tempfile::tempdir().unwrap();
    picture(dir.path(), "a.png");
    let Some((mut state, woken, later)) = watched(dir.path()) else { return };
    state.pause();
    assert!(state.listed.watch.is_none() && state.listed.scanning.is_none());
    let before = woken.load(Ordering::SeqCst);
    picture(dir.path(), "c.png");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(woken.load(Ordering::SeqCst), before, "a change woke the daemon while hidden");
    assert_eq!(state.listing_wake(later), Wake::At(0.0), "shown again, the folder is looked at at once");
    assert!(listed(&mut state, later) && state.listed.text.contains("c.png"));
    assert!(state.listed.watch.is_some(), "the watch comes back when shown");
}

#[test]
fn a_folder_removed_and_made_again_is_watched_again() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("album");
    std::fs::create_dir(&dir).unwrap();
    picture(&dir, "a.png");
    let Some((mut state, _, later)) = watched(&dir) else { return };
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(told(&state), "removing the folder sent no notice");
    assert!(listed(&mut state, later) && !state.listed.text.contains("a.png"));
    assert!(state.watching(), "a removed folder is awaited in the folder above it, not looked at every 2 s");
    std::fs::create_dir(&dir).unwrap();
    assert!(told(&state), "making the folder again sent no notice");
    picture(&dir, "d.png");
    let shown = (0..400).any(|_| {
        state.listing(later);
        let whole = state.listed.watch.as_ref().is_some_and(|w| w.whole());
        (whole && state.listed.text.contains("d.png"))
            || (std::thread::sleep(Duration::from_millis(5)), false).1
    });
    assert!(shown, "the folder made again is watched and listed again");
}

#[test]
fn only_the_folders_a_recursive_look_reaches_are_read() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sub/deeper")).unwrap();
    std::fs::create_dir(dir.path().join(".hidden")).unwrap();
    assert_eq!(watch::dirs(dir.path(), 0), (vec![dir.path().to_owned()], false));
    let (one, _) = watch::dirs(dir.path(), 1);
    assert!(one.len() == 2 && one.contains(&dir.path().join("sub")));
    assert_eq!(watch::dirs(dir.path(), 2).0.len(), 3);
    assert_eq!(watch::dirs(&dir.path().join("missing"), 2), (vec![dir.path().join("missing")], false));
}

#[test]
fn hidden_files_wake_nothing_and_a_burst_of_changes_settles_before_a_look() {
    let dir = tempfile::tempdir().unwrap();
    picture(dir.path(), "a.png");
    let Some((mut state, _, later)) = watched(dir.path()) else { return };
    std::fs::write(dir.path().join(".partial"), b"x").unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let pending = |state: &SceneState| state.listed.watch.as_ref().and_then(|w| w.pending(Instant::now()));
    assert_eq!(pending(&state), None, "a hidden file woke the listing");
    picture(dir.path(), "b.png");
    assert!(told(&state), "no change notice arrived");
    let wait = pending(&state).unwrap_or_default();
    assert!(wait > 0.0 && wait <= 0.5, "the look waits for the copy to settle: {wait}");
    state.listing(Instant::now());
    assert!(state.listed.scanning.is_none(), "a look started while files were still arriving");
    assert!(listed(&mut state, later) && state.listed.text.contains("b.png"));
}

#[test]
fn unreadable_subfolders_and_too_many_folders_are_told_apart() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let open = std::fs::read_dir(&locked).is_ok();
    let found = watch::Watch::new(watch::dirs(dir.path(), 1), Arc::new(|| {}));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    let Some(watch) = found else { return eprintln!("no change notices on this system") };
    assert!(open || (watch.trusted() && watch.whole()), "a folder nobody may read made the watch untrusted");
    (0..=watch::MOST).for_each(|i| std::fs::create_dir(dir.path().join(format!("d{i}"))).unwrap());
    let (dirs, cut) = watch::dirs(dir.path(), 1);
    assert!(cut && dirs.len() == watch::MOST);
    let many = watch::Watch::new((dirs, cut), Arc::new(|| {}));
    assert!(many.is_none_or(|w| !w.trusted()), "a cut list of folders was trusted");
}

#[test]
fn only_network_fuse_mounts_count_as_remote() {
    let info = "36 25 0:32 / /media/disk rw - fuseblk /dev/sdb1 rw\n\
                37 25 0:33 / /mnt/sky rw - fuse.sshfs host:/ rw\n\
                38 25 0:34 / /srv/vault rw - fuse.gocryptfs vault rw\n\
                39 25 0:35 / /mnt/my\\040drive rw - fuse.rclone remote: rw\n";
    assert!(!super::mounts::fuse(std::path::Path::new("/media/disk/photos"), info));
    assert!(super::mounts::fuse(std::path::Path::new("/mnt/sky/album"), info));
    assert!(!super::mounts::fuse(std::path::Path::new("/srv/vault/album"), info));
    assert!(super::mounts::fuse(std::path::Path::new("/mnt/my drive/album"), info));
}
