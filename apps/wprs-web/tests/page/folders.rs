use super::helper::{Page, available, start};
use serde_json::json;
use wprs_web::{Folder, Isolation, ToWeb, token};

#[test]
fn a_file_changed_in_place_is_told_as_changed() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let mut page =
        Page::open(start(dir.path(), 9), Isolation { network: true, devices: false, limits: false });
    assert_eq!(page.wait("files", 20).as_array().map(Vec::len), Some(1));
    let picture = dir.path().join("photo.png");
    let old = token("photos", &[7; 32], &picture);
    std::fs::write(&picture, b"\x89PNG\r\n\x1a\nmore").unwrap();
    let new = token("photos", &[7; 32], &picture);
    assert_ne!(old, new);
    let files = vec![(new.clone(), picture)];
    page.link.send(&ToWeb::Folder(Folder { prop: "photos".into(), all: true, files }), &[]).unwrap();
    let want = json!({"prop": "photos", "added": [], "removed": [], "changed": [[old, new]]});
    assert_eq!(page.wait("change", 10), want);
}
