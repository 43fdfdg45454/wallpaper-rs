use std::fs;
use std::path::Path;
use tempfile::TempDir;
use wprs_library::{Error, Library, Meta};

const NOW: i64 = 1_700_000_000;
const DAY: i64 = 86_400;

struct Fixture {
    library: Library,
    source: String,
    _dir: TempDir,
}

fn write(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, path.file_name().unwrap().as_encoded_bytes()).unwrap();
}

fn fixture() -> Fixture {
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

fn titles(library: &Library, query: &str) -> Vec<String> {
    let mut titles: Vec<String> = library
        .search(query, NOW, 0, 100)
        .unwrap_or_else(|e| panic!("{query}: {e}"))
        .into_iter()
        .map(|i| i.title)
        .collect();
    titles.sort();
    titles
}

fn ordered(library: &Library, query: &str, offset: u32, limit: u32) -> Vec<String> {
    library.search(query, NOW, offset, limit).unwrap().into_iter().map(|i| i.title).collect()
}

#[test]
fn every_key_filters_items() {
    let Fixture { library, source, .. } = fixture();
    let source_query = format!("source:{source}");
    let cases: [(&str, &[&str]); 47] = [
        ("", &["café", "forest", "night", "ocean waves", "square"]),
        ("waves", &["ocean waves"]),
        ("oce wav", &["ocean waves"]),
        ("\"ocean waves\"", &["ocean waves"]),
        ("\"waves ocean\"", &[]),
        ("-waves", &["café", "forest", "night", "square"]),
        ("tag:sea", &["night", "ocean waves"]),
        ("tag:SEA tag:blue", &["ocean waves"]),
        ("-tag:sea", &["café", "forest", "square"]),
        ("tag:\"sea\"", &["night", "ocean waves"]),
        ("kind:video", &["forest", "night"]),
        ("kind:image", &["café", "ocean waves", "square"]),
        ("kind:package", &[]),
        ("fav:", &["café"]),
        ("fav:no", &["forest", "night", "ocean waves", "square"]),
        ("rating:>=4", &["café", "night"]),
        ("rating:5", &["night"]),
        ("rating:=4", &["café"]),
        ("rating:>4", &["night"]),
        ("rating:<3", &["forest", "ocean waves", "square"]),
        ("rating:<=2", &["forest", "ocean waves", "square"]),
        ("res:4k", &["café"]),
        ("res:1440p", &["café", "forest"]),
        ("res:>=1920x1080", &["café", "forest", "ocean waves"]),
        ("res:<1920x1080", &["square"]),
        ("orient:portrait", &["night"]),
        ("orient:landscape", &["café", "forest", "ocean waves"]),
        ("orient:square", &["square"]),
        ("dur:<30s", &["night"]),
        ("dur:>1m", &["forest"]),
        ("dur:<=2m", &["forest", "night"]),
        ("audio:yes", &["night"]),
        ("audio:no", &["forest"]),
        (&source_query, &["café", "night", "ocean waves", "square"]),
        ("in:Picks", &["forest", "ocean waves"]),
        ("in:picks -kind:video", &["ocean waves"]),
        ("added:<7d", &["forest"]),
        ("added:>7d", &["café", "night", "ocean waves", "square"]),
        ("used:never", &["café", "night", "ocean waves", "square"]),
        ("used:<1h", &["forest"]),
        ("missing:", &["square"]),
        ("-missing:", &["café", "forest", "night", "ocean waves"]),
        ("color:#3366ff", &["café"]),
        ("color:#3366f0", &["café"]),
        ("color:#000000", &["night", "ocean waves"]),
        ("color:#ff0000", &[]),
        ("Kind:VIDEO -tag:sea", &["forest"]),
    ];
    for (query, expected) in cases {
        assert_eq!(titles(&library, query), expected, "{query}");
        assert_eq!(library.count(query, NOW).unwrap() as usize, expected.len(), "{query}");
    }
}

#[test]
fn free_words_ignore_diacritics_and_match_prefixes_of_titles_tags_and_paths() {
    let Fixture { library, .. } = fixture();
    for query in ["cafe", "CAFÉ", "caf", "warm", "pictures café", "jpg"] {
        assert_eq!(titles(&library, query), ["café"], "{query}");
    }
    assert_eq!(titles(&library, "loose"), ["forest"]);
}

#[test]
fn sorting_and_paging() {
    let Fixture { library, .. } = fixture();
    assert_eq!(ordered(&library, "sort:title", 0, 10), ["café", "forest", "night", "ocean waves", "square"]);
    assert_eq!(ordered(&library, "sort:title-", 0, 2), ["square", "ocean waves"]);
    assert_eq!(ordered(&library, "sort:title", 1, 2), ["forest", "night"]);
    assert_eq!(ordered(&library, "sort:rating-", 0, 3), ["night", "café", "ocean waves"]);
    assert_eq!(ordered(&library, "sort:added-", 0, 1), ["forest"]);
    assert_eq!(ordered(&library, "sort:used-", 0, 1), ["forest"]);
    assert_eq!(ordered(&library, "", 0, 1), ["forest"]);
    let mut shuffled = ordered(&library, "sort:random", 0, 10);
    shuffled.sort();
    assert_eq!(shuffled, titles(&library, ""));
    assert_eq!(library.count("sort:title kind:video", NOW).unwrap(), 2);
}

#[test]
fn injection_looking_input_is_only_ever_data() {
    let Fixture { library, .. } = fixture();
    let attempts = [
        "tag:'); DROP TABLE items;--",
        "\"'); DROP TABLE items;--\"",
        "in:x' OR '1'='1",
        "source:\"x' OR 1=1 --\"",
        "tag:\"sea' OR 'a'='a\"",
        "'; DELETE FROM items; --",
        "NEAR(a b) OR x*",
    ];
    for query in attempts {
        assert_eq!(titles(&library, query), Vec::<String>::new(), "{query}");
    }
    assert_eq!(library.count("", NOW).unwrap(), 5);
    assert_eq!(library.tags().unwrap().len(), 3);
}

#[test]
fn unknown_keys_and_bad_values_are_clear_errors() {
    let Fixture { library, .. } = fixture();
    let error = library.search("size:big", NOW, 0, 10).unwrap_err();
    assert!(
        error.to_string().starts_with("invalid query: unknown key \"size:\", expected one of tag, kind"),
        "{error}"
    );
    let error = library.count("rating:9", NOW).unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid query: invalid value in \"rating:9\", expected a rating from 0 to 5 with an optional >=, <=, >, < or ="
    );
    let bad = [
        "kind:gif",
        "res:big",
        "dur:soon",
        "color:blue",
        "color:#12345",
        "orient:diagonal",
        "audio:",
        "tag:",
        "fav:maybe",
        "added:7",
        "sort:size",
        "-sort:title",
        "\"unclosed",
    ];
    for query in bad {
        assert!(matches!(library.search(query, NOW, 0, 10), Err(Error::Query(_))), "{query}");
    }
}
