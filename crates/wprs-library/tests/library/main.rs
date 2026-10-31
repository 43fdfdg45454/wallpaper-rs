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

mod items;
mod kept;
mod playlists;
mod storage;

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
