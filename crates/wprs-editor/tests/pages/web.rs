use super::{act, manifest, page, state};
use serde_json::json;
use std::fs;
use wprs_editor::Project;
use wprs_editor::package::{build_saved, extract, problems};
use wprs_editor::pages::{files, scened};
use wprs_editor::starters::starter;

#[test]
fn the_web_starter_makes_a_page_the_editor_opens_and_builds() {
    let root = tempfile::tempdir().unwrap();
    let mut project = page(root.path());
    let dir = project.dir.clone();
    assert!(dir.join("index.html").is_file() && dir.join("app.js").is_file());
    assert!(!dir.join("scene.json").exists() && !dir.join("assets").exists());
    assert!(!scened(&project.doc) && project.doc.get("scene").is_none());
    assert_eq!(project.doc["manifest"]["requires"]["features"], json!(["engine_strings", "web"]));
    assert!(problems(&dir, &project.doc).iter().all(|p| p.warning), "{:?}", problems(&dir, &project.doc));
    let listed = files(&dir, &project.doc);
    let role = |path: &str| listed.iter().find(|f| f["path"] == path).map(|f| f["role"].clone());
    assert_eq!((role("index.html"), role("app.js")), (Some(json!("entry")), Some(json!("file"))));
    act(&mut project, json!({"set": {"pointer": "/manifest/title", "value": "Clock"}})).unwrap();
    project.save().unwrap();
    let again = Project::open(&dir).unwrap();
    assert_eq!((again.doc["manifest"]["title"].as_str(), again.dirty()), (Some("Clock"), false));
    let out = root.path().join("page.wpak");
    build_saved(&dir, &again.doc, &out, None).unwrap();
    let copy = root.path().join("copy");
    assert_eq!(extract(&out, &copy).unwrap(), "org.example.page.copy");
    let copied = Project::open(&copy).unwrap();
    assert!(copied.dir.join("index.html").is_file() && copied.doc["manifest"]["kind"] == "web");
}

#[test]
fn a_missing_entry_or_preload_is_a_problem() {
    let root = tempfile::tempdir().unwrap();
    let mut project = page(root.path());
    act(&mut project, json!({"set": {"pointer": "/manifest/entry", "value": "gone.html"}})).unwrap();
    let found = problems(&project.dir, &project.doc);
    assert!(found.iter().any(|p| p.pointer == "/manifest/entry" && !p.warning), "{found:?}");
    act(&mut project, json!({"undo": {}})).unwrap();
    let refused = manifest(&mut project, json!({"web": {"values": {"preload": "gone.js"}}}));
    assert!(refused.unwrap_err().to_string().contains("gone.js"));
}

#[test]
fn files_are_copied_into_a_page_but_never_over_the_manifest() {
    let root = tempfile::tempdir().unwrap();
    let mut project = page(root.path());
    let file = root.path().join("extra.js");
    fs::write(&file, "1").unwrap();
    manifest(&mut project, json!({"add_file": {"file": file, "to": "lib"}})).unwrap();
    assert!(project.dir.join("lib/extra.js").is_file());
    let manifest_file = root.path().join("manifest.json");
    fs::write(&manifest_file, "{}").unwrap();
    assert!(manifest(&mut project, json!({"add_file": {"file": manifest_file}})).is_err());
    assert!(manifest(&mut project, json!({"add_file": {"file": file, "to": "../out"}})).is_err());
    state();
    let mut scene =
        Project::create(&root.path().join("scene"), starter("empty", "org.example.s", "S", "en").unwrap())
            .unwrap();
    assert!(manifest(&mut scene, json!({"add_file": {"file": file}})).is_err());
}
