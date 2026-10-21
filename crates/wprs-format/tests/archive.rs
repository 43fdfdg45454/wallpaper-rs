mod common;

use common::{archive, manifest, package, patch, put, zip};
use std::io::Cursor;
use wprs_format::{Error, Kind, Limits, Package, Result, read};
use zip::write::SimpleFileOptions;

fn load(bytes: Vec<u8>) -> Result<Package<Cursor<Vec<u8>>>> {
    read(Cursor::new(bytes), &Limits::default())
}

#[test]
fn reads_a_valid_package_in_place() {
    let manifest = manifest(Kind::Image);
    let mut package = load(package(&manifest, &[("assets/", b""), ("assets/extra.png", b"extra")])).unwrap();
    assert_eq!(package.manifest, manifest);
    let paths: Vec<&str> = package.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["manifest.json", "image.png", "preview.webp", "assets/extra.png"]);
    assert_eq!(package.read_file("assets/extra.png").unwrap(), b"extra");
    assert!(matches!(package.read_file("assets/none.png"), Err(Error::Missing(_))));
}

#[test]
fn zip_slip_entries_are_rejected() {
    let names =
        ["../evil.png", "assets/../../evil.png", "/tmp/evil.png", "C:/evil.png", "..\\evil.png", "../up/"];
    for name in names {
        let result = load(package(&manifest(Kind::Image), &[(name, b"x")]));
        assert!(matches!(result, Err(Error::UnsafePath(_))), "{name:?}: {result:?}");
    }
}

#[test]
fn zip_bombs_are_rejected() {
    let zeros = vec![0u8; 8 << 20];
    let result = load(package(&manifest(Kind::Image), &[("assets/bomb.bmp", &zeros)]));
    assert!(matches!(result, Err(Error::Limit { what: "compression ratio", .. })), "{result:?}");
    let limits = Limits { max_file_size: 1 << 20, max_ratio: u64::MAX, ..Limits::default() };
    let result = read(Cursor::new(package(&manifest(Kind::Image), &[("assets/big.bmp", &zeros)])), &limits);
    assert!(matches!(result, Err(Error::Limit { what: "file size", .. })), "{result:?}");
}

#[test]
fn too_many_entries_are_rejected() {
    let names: Vec<String> = (0..10).map(|i| format!("assets/{i}.png")).collect();
    let extra: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), b"x".as_slice())).collect();
    let limits = Limits { max_entries: 8, ..Limits::default() };
    let result = read(Cursor::new(package(&manifest(Kind::Image), &extra)), &limits);
    assert!(matches!(result, Err(Error::Limit { what: "entry count", actual: 13, limit: 8 })), "{result:?}");
}

#[test]
fn duplicate_entries_are_rejected() {
    let bytes = package(&manifest(Kind::Image), &[("assets/a.png", b"a"), ("assets/b.png", b"b")]);
    let result = load(patch(bytes, b"assets/b.png", b"assets/a.png"));
    assert!(matches!(&result, Err(Error::Zip(e)) if e.to_string().contains("duplicate")), "{result:?}");
    let result = load(package(&manifest(Kind::Image), &[("assets/a.png", b"a"), ("assets/A.png", b"b")]));
    assert!(matches!(result, Err(Error::Duplicate(_))), "{result:?}");
}

#[test]
fn symlink_entries_are_rejected() {
    let json = manifest(Kind::Image).to_json().unwrap();
    let files: [(&str, &[u8]); 3] =
        [("manifest.json", json.as_bytes()), ("image.png", b"x"), ("preview.webp", b"x")];
    let bytes = zip(|z| {
        files.into_iter().for_each(|(name, data)| put(z, name, data));
        z.add_symlink("assets/link.png", "/etc/passwd", SimpleFileOptions::default()).unwrap();
    });
    assert!(matches!(load(bytes), Err(Error::Symlink(path)) if path == "assets/link.png"));
}

#[test]
fn broken_packages_are_rejected() {
    let missing = load(archive(&[("image.png", b"x")]));
    assert!(matches!(missing, Err(Error::Missing(ref path)) if path == "manifest.json"), "{missing:?}");
    let malformed = load(archive(&[("manifest.json", b"{\"schema\":"), ("image.png", b"x")]));
    assert!(matches!(malformed, Err(Error::Json(_))), "{malformed:?}");
    let mut without_entry = manifest(Kind::Image);
    without_entry.entry = "other.png".into();
    let json = without_entry.to_json().unwrap();
    let result = load(archive(&[("manifest.json", json.as_bytes()), ("preview.webp", b"x")]));
    assert!(matches!(result, Err(Error::Missing(ref path)) if path == "other.png"), "{result:?}");
    let mut bad_id = manifest(Kind::Image);
    bad_id.id = "Not A Valid Id".into();
    assert!(matches!(load(package(&bad_id, &[])), Err(Error::Invalid { field, .. }) if field == "id"));
    assert!(matches!(load(b"not a zip archive".to_vec()), Err(Error::Zip(_))));
}
