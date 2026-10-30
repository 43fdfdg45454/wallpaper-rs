use std::fs::{File, OpenOptions, TryLockError};
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) fn me() -> &'static str {
    static ME: OnceLock<String> = OnceLock::new();
    ME.get_or_init(|| {
        let seed = std::collections::hash_map::RandomState::new();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        format!("{}.{:016x}", std::process::id(), seed.hash_one((nanos, std::process::id())))
    })
}

pub(crate) fn dir(book: &Path) -> PathBuf {
    book.with_extension("trials")
}

pub(crate) fn make(dir: &Path) -> std::io::Result<()> {
    let mut dirs = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut dirs, 0o700);
    dirs.recursive(true).create(dir)
}

pub(crate) fn hold(book: &Path) -> std::io::Result<File> {
    let dir = dir(book);
    make(&dir)?;
    let file = OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(me()))?;
    file.try_lock().map_err(std::io::Error::from)?;
    Ok(file)
}

fn named(token: &str) -> bool {
    !token.starts_with('.') && token.len() <= 64 && token.bytes().all(|b| b.is_ascii_hexdigit() || b == b'.')
}

pub(crate) fn alive(book: &Path, token: &str) -> bool {
    if token == me() {
        return true;
    }
    let path = dir(book).join(token);
    let Some(file) = Some(token).filter(|t| named(t)).and_then(|_| File::open(&path).ok()) else {
        return false;
    };
    match file.try_lock() {
        Err(TryLockError::WouldBlock) => true,
        _ => {
            drop(std::fs::remove_file(&path));
            false
        }
    }
}

pub(crate) fn sweep(book: &Path) {
    let found = std::fs::read_dir(dir(book)).into_iter().flatten().flatten();
    for token in found.filter_map(|e| e.file_name().into_string().ok()) {
        alive(book, &token);
    }
}
