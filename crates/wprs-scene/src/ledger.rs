use crate::book::{Line, STRIKES, parse, save, text};
use crate::trials::{alive, make, sweep};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

type Stamp = Option<(u64, Option<SystemTime>, u64)>;

pub(crate) struct Book {
    pub(crate) path: Option<PathBuf>,
    pub(crate) seen: Option<Stamp>,
    pub(crate) lines: Vec<Line>,
    pub(crate) mine: Vec<u64>,
    pub(crate) held: Option<File>,
}

static BOOK: Mutex<Book> =
    Mutex::new(Book { path: None, seen: Some(None), lines: Vec::new(), mine: Vec::new(), held: None });

pub(crate) fn unread() -> MutexGuard<'static, Book> {
    BOOK.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn book() -> MutexGuard<'static, Book> {
    let mut book = unread();
    if let Some(path) = book.path.clone() {
        let stamp = stamp(&path);
        if book.seen != Some(stamp) {
            let found = std::fs::read_to_string(&path).unwrap_or_default();
            book.lines = parse(&found, now(), &|token| alive(&path, token));
            book.seen = Some(stamp);
        }
    }
    book
}

fn stamp(path: &Path) -> Stamp {
    let found = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    let node = std::os::unix::fs::MetadataExt::ino(&found);
    #[cfg(not(unix))]
    let node = 0;
    Some((found.len(), found.modified().ok(), node))
}

pub(crate) fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub(crate) fn blocked(lines: &[Line], key: u64) -> bool {
    lines.iter().any(|l| matches!(l, Line::Struck(k, s, _) if *k == key && *s >= STRIKES))
}

pub(crate) fn locked<T>(path: &Path, work: impl FnOnce() -> T) -> std::io::Result<T> {
    path.parent().map_or(Ok(()), make)?;
    let lock = path.with_extension("lock");
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(lock)?;
    lock.lock()?;
    Ok(work())
}

pub(crate) fn edit(book: &mut Book, change: impl Fn(&mut Vec<Line>)) -> std::io::Result<()> {
    let Some(path) = book.path.clone() else {
        change(&mut book.lines);
        return Ok(());
    };
    let written = locked(&path, || {
        let found = std::fs::read_to_string(&path).unwrap_or_default();
        let mut lines = parse(&found, now(), &|token| alive(&path, token));
        change(&mut lines);
        sweep(&path);
        let saved = if text(&lines) == found { Ok(()) } else { save(&path, &lines) };
        saved.map(|()| (lines, stamp(&path)))
    });
    match written.and_then(|written| written) {
        Ok((lines, stamp)) => {
            (book.lines, book.seen) = (lines, Some(stamp));
            Ok(())
        }
        Err(e) => {
            change(&mut book.lines);
            Err(e)
        }
    }
}
