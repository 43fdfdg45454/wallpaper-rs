use super::{NOW, package};
use serde_json::json;
use std::fs;
use tempfile::TempDir;
use wprs_format::{Limits, Rating};
use wprs_library::Library;

#[test]
fn assets_packages_are_listed_apart_from_wallpapers() {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let root = dir.path().join("pictures");
    package(&root, "meadow", Some(Rating::General), false);
    let source = dir.path().join("parts");
    fs::create_dir_all(&source).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.parts", "version": "1.0.0",
        "kind": "assets", "entry": "assets.json", "title": "Parts", "license": "CC0-1.0",
        "origin": { "kind": "original" }, "content": { "rating": "general" }, "photosensitive": false,
        "requires": { "features": ["assets"] } });
    let scene = json!({ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 64] },
        "layers": [{ "id": "glow", "type": "solid", "color": "#ffcc00" }] });
    fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
    fs::write(source.join("assets.json"), scene.to_string()).unwrap();
    wprs_format::build(&source, root.join("parts.wpak"), &Limits::default()).unwrap();
    let scanned = library.add_folder(&root, true).unwrap();
    library.scan(&scanned, NOW).unwrap();
    let count = |q: &str| library.count(q, NOW).unwrap();
    assert_eq!(["", "kind:assets", "-kind:assets", "kind:package", "parts"].map(count), [1, 1, 1, 2, 0]);
    let found = library.search("kind:assets", NOW, 0, 5).unwrap();
    assert_eq!(found[0].package.as_ref().unwrap().kind, "assets");
    assert!(library.count("kind:bundle", NOW).is_err());
}
