use std::fs;
use std::path::Path;
use tempfile::TempDir;
use wprs_format::{Limits, Manifest, SCHEMA_V1};
use wprs_library::Library;

const NOW: i64 = 1_700_000_000;

fn package(dir: &Path, tags: &[&str], pixels: &str) {
    let manifest = Manifest {
        schema: SCHEMA_V1.into(),
        id: "org.example.calm".into(),
        version: "1.0.0".into(),
        title: "Calm Water".into(),
        license: "CC0-1.0".into(),
        kind: wprs_format::Kind::Image,
        entry: "image.png".into(),
        tags: tags.iter().map(ToString::to_string).collect(),
        ..Default::default()
    };
    let source = dir.join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    fs::write(source.join("image.png"), pixels).unwrap();
    fs::create_dir_all(dir.join("pictures")).unwrap();
    wprs_format::build(&source, dir.join("pictures/calm.wpak"), &Limits::default()).unwrap();
}

#[test]
fn tags_the_user_deleted_stay_deleted_when_the_package_file_changes() {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    package(dir.path(), &["water", "night", "blue"], "1");
    let source = library.add_folder(dir.path().join("pictures"), false).unwrap();
    library.scan(&source, NOW).unwrap();
    let uid = library.search("", NOW, 0, 10).unwrap()[0].uid.clone();
    let tags = |library: &Library| library.item(&uid).unwrap().tags;
    library.retag(&[&uid], &["mine"], &["WATER", "never had"]).unwrap();
    library.rename_tag("night", "dark").unwrap();
    assert_eq!(tags(&library), ["blue", "dark", "mine"]);
    package(dir.path(), &["water", "night", "blue", "fresh"], "22");
    assert_eq!(library.scan(&source, NOW + 60).unwrap().updated, 1);
    assert_eq!(tags(&library), ["blue", "dark", "fresh", "mine"]);
    assert!(library.search("tag:water", NOW, 0, 10).unwrap().is_empty());
    library.retag(&[&uid], &["Water"], &["blue"]).unwrap();
    library.rename_tag("dark", "Night").unwrap();
    package(dir.path(), &["water", "night", "blue", "fresh", "never had"], "333");
    library.scan(&source, NOW + 120).unwrap();
    assert_eq!(tags(&library), ["fresh", "mine", "never had", "Night", "water"]);
    library.remove_items(&[&uid]).unwrap();
    library.scan(&source, NOW + 180).unwrap();
    let uid = library.search("", NOW, 0, 10).unwrap()[0].uid.clone();
    assert_eq!(library.item(&uid).unwrap().tags, ["blue", "fresh", "never had", "Night", "water"]);
}
