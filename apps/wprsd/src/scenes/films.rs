use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

const KEPT: usize = 64;

type Stamp = Option<SystemTime>;
type Kept = HashMap<PathBuf, (Stamp, Option<f32>)>;

static FILMED: LazyLock<Mutex<Kept>> = LazyLock::new(Default::default);

fn kept() -> MutexGuard<'static, Kept> {
    FILMED.lock().unwrap_or_else(PoisonError::into_inner)
}

fn stamp(path: &Path) -> Stamp {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub(crate) fn measure(path: &Path) {
    let at = stamp(path);
    if kept().get(path).is_some_and(|(s, _)| *s == at) {
        return;
    }
    let seconds = wprs_scene::film_file(path);
    let mut kept = kept();
    if kept.len() >= KEPT {
        kept.clear();
    }
    kept.insert(path.to_owned(), (at, seconds));
}

pub(crate) fn measured(path: &Path) -> Option<f32> {
    kept().get(path).and_then(|(_, seconds)| *seconds)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn lengths_are_read_once_where_the_daemon_picks_a_video_and_only_looked_up_while_drawing() {
        let dir = tempfile::tempdir().unwrap();
        let film = dir.path().join("sea.webm");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/magenta-64x32.webm");
        std::fs::copy(fixture, &film).unwrap();
        assert_eq!(super::measured(&film), None, "drawing never reads the file itself");
        super::measure(&film);
        assert!(super::measured(&film).is_some_and(|s| s > 0.0));
        std::fs::write(&film, b"not a video").unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options().write(true).open(&film).unwrap().set_modified(later).unwrap();
        super::measure(&film);
        assert_eq!(super::measured(&film), None, "a replaced file is measured again");
    }
}
