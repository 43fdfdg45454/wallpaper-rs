mod keys;
mod order;

use rusqlite::Connection;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use wprs_library::{Library, Meta};

pub const NOW: i64 = 1_700_000_000;
pub const DAY: i64 = 86_400;

pub struct Fixture {
    pub library: Library,
    pub source: String,
    pub _dir: TempDir,
}

fn write(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, path.file_name().unwrap().as_encoded_bytes()).unwrap();
}

pub fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let root = dir.path().join("pictures");
    ["café.jpg", "ocean waves.png", "night.webm", "square.webp"]
        .iter()
        .for_each(|name| write(&root.join(name)));
    let source = library.add_folder(&root, true).unwrap();
    library.scan(&source, NOW - 30 * DAY).unwrap();
    fs::remove_file(root.join("square.webp")).unwrap();
    library.scan(&source, NOW - 30 * DAY).unwrap();
    write(&dir.path().join("loose/forest.mp4"));
    library.import_file(dir.path().join("loose/forest.mp4"), NOW).unwrap();
    let uid = |title: &str| {
        library.search("", NOW, 0, 10).unwrap().into_iter().find(|i| i.title == title).unwrap().uid
    };
    let [cafe, ocean, night, square, forest] = ["café", "ocean waves", "night", "square", "forest"].map(uid);
    let meta = |width, height, duration_ms, audio, color| Meta {
        width: Some(width),
        height: Some(height),
        duration_ms,
        audio,
        color: Some(color),
        error: None,
    };
    library.set_meta(&cafe, &meta(3840, 2160, None, None, 0x3366ff)).unwrap();
    library.set_meta(&ocean, &meta(1920, 1080, None, None, 0x000000)).unwrap();
    library.set_meta(&night, &meta(1080, 1920, Some(20_000), Some(true), 0x101020)).unwrap();
    library.set_meta(&forest, &meta(2560, 1440, Some(120_000), Some(false), 0x206020)).unwrap();
    library.set_meta(&square, &meta(1000, 1000, None, None, 0xffffff)).unwrap();
    [(&cafe, 4), (&ocean, 2), (&night, 5)]
        .into_iter()
        .for_each(|(uid, n)| library.set_rating(uid, n).unwrap());
    library.set_favorite(&cafe, true).unwrap();
    [(&cafe, "warm"), (&ocean, "sea"), (&ocean, "blue"), (&night, "sea")]
        .into_iter()
        .for_each(|(uid, tag)| library.add_tag(uid, tag).unwrap());
    library.mark_used(&forest, "DP-1", NOW - 100).unwrap();
    let picks = library.create_playlist("Picks", None, &Default::default()).unwrap();
    library.add_to_playlist(&picks, &[&ocean, &forest]).unwrap();
    Fixture { library, source, _dir: dir }
}

pub fn titles(library: &Library, query: &str) -> Vec<String> {
    let mut titles: Vec<String> = library
        .search(query, NOW, 0, 100)
        .unwrap_or_else(|e| panic!("{query}: {e}"))
        .into_iter()
        .map(|i| i.title)
        .collect();
    titles.sort();
    titles
}

pub fn ordered(library: &Library, query: &str, offset: u32, limit: u32) -> Vec<String> {
    library.search(query, NOW, offset, limit).unwrap().into_iter().map(|i| i.title).collect()
}
