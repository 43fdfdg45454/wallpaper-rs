use image::{ImageFormat, RgbImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wprs_editor::actions::{Action, ImportAssets};
use wprs_editor::offer::{Offer, make};
use wprs_editor::shelf::Pick;
use wprs_editor::starters::starter;
use wprs_editor::{Project, package, stock};

fn picture(path: &Path, color: [u8; 3]) {
    RgbImage::from_pixel(64, 32, color.into()).save_with_format(path, ImageFormat::Png).unwrap();
}

fn source(root: &Path) -> Project {
    let mut project = Project::create(
        &root.join("source"),
        starter("empty", "org.example.source", "Source", "en").unwrap(),
    )
    .unwrap();
    picture(&root.join("sky.png"), [40, 90, 160]);
    project.act(&Action::Import { file: root.join("sky.png"), id: Some("sky".into()) }).unwrap();
    let layer = |kind: &str, id: &str| Action::AddLayer {
        kind: kind.into(),
        id: Some(id.into()),
        name: None,
        source: None,
        into: None,
        at: None,
    };
    project.act(&layer("image", "glow")).unwrap();
    project.act(&layer("particles", "snow")).unwrap();
    project.act(&layer("solid", "left")).unwrap();
    project.save().unwrap();
    project
}

fn shared(root: &Path) -> PathBuf {
    let from = source(root);
    let picks: Vec<Pick> = ["layer:glow", "particles:snow"].iter().map(|t| Pick::parse(t).unwrap()).collect();
    let offer = Offer {
        id: "org.example.parts".into(),
        title: "Parts".into(),
        license: "CC-BY-4.0".into(),
        attribution: "Example Maker".into(),
        thumbnail: Some(root.join("sky.png")),
        thumbnails: BTreeMap::from([("layer:glow".into(), root.join("sky.png"))]),
        titles: BTreeMap::from([("layer:glow".into(), "Soft glow".into())]),
    };
    let out = root.join("parts");
    let made = make(&from.dir, &from.doc, &picks, &out, &offer).unwrap();
    assert_eq!(made.doc["manifest"]["kind"], "assets");
    assert!(made.doc["scene"]["layers"].as_array().unwrap().iter().all(|l| l["id"] != "left"));
    assert_eq!(made.doc["scene"]["ext"]["parts"][0]["title"], "Soft glow");
    assert!(out.join("parts/0.jpg").is_file() && out.join("preview.jpg").is_file());
    let wpak = root.join("parts.wpak");
    package::build_saved(&made.dir, &made.doc, &wpak, None).unwrap();
    wpak
}

fn target(root: &Path, id: &str) -> Project {
    Project::create(&root.join(id), starter("empty", &format!("org.example.{id}"), id, "en").unwrap())
        .unwrap()
}

#[test]
fn asset_packages_are_made_from_chosen_parts_and_imported_with_credits() {
    let root = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_STATE_HOME", root.path().join("state")) };
    let wpak = shared(root.path());
    let cache = root.path().join("cache");
    let listed = stock::browse(&wpak, Some(&cache)).unwrap();
    assert_eq!(listed["parts"].as_array().unwrap().len(), 2);
    assert_eq!(listed["parts"][0]["files"][0]["license"], "CC-BY-4.0");
    assert!(Path::new(listed["parts"][0]["thumbnail"].as_str().unwrap()).is_file());
    let mut project = target(root.path(), "scene");
    project
        .act(&Action::AddLayer {
            kind: "solid".into(),
            id: Some("glow".into()),
            name: None,
            source: None,
            into: None,
            at: None,
        })
        .unwrap();
    let wanted = ImportAssets { package: wpak.clone(), parts: vec!["layer:glow".into()] };
    project.act(&Action::ImportAssets(wanted)).unwrap();
    let doc = &project.doc;
    let layers: Vec<&Value> = doc["scene"]["layers"].as_array().unwrap().iter().map(|l| &l["id"]).collect();
    assert_eq!(layers, [&json!("glow"), &json!("glow_2")]);
    let path = doc["scene"]["assets"]["sky"]["path"].as_str().unwrap();
    assert!(path.starts_with("assets/org.example.parts-1.0.0/"), "{path}");
    assert!(project.dir.join(path).is_file());
    let credit = &doc["manifest"]["credits"][0];
    assert_eq!(credit["source"], json!({ "id": "org.example.parts", "version": "1.0.0" }));
    assert_eq!(
        (credit["license"].as_str(), credit["attribution"].as_str()),
        (Some("CC-BY-4.0"), Some("Example Maker"))
    );
    assert_eq!(doc["manifest"]["origin"]["kind"], "derived");
    assert_eq!(doc["manifest"]["origin"]["derived_from"][0]["title"], "Parts");
    project.act(&Action::Undo {}).unwrap();
    assert!(project.doc["scene"]["assets"].get("sky").is_none());
    assert!(project.doc["manifest"].get("credits").is_none());
    project.act(&Action::Redo {}).unwrap();
    assert_eq!(project.doc["manifest"]["credits"].as_array().unwrap().len(), 1);
    let all = ImportAssets { package: wpak.clone(), parts: Vec::new() };
    project.act(&Action::ImportAssets(all)).unwrap();
    assert!(project.doc["scene"]["particles"].get("snow").is_some());
    assert_eq!(project.doc["manifest"]["credits"].as_array().unwrap().len(), 1);
    project.save().unwrap();
    assert!(package::problems(&project.dir, &project.doc).iter().all(|p| p.warning));
}

#[test]
fn bad_imports_and_offers_are_refused() {
    let root = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_STATE_HOME", root.path().join("state")) };
    let wpak = shared(root.path());
    let mut project = target(root.path(), "scene");
    let import = |p: &mut Project, package: &Path, parts: &[&str]| {
        let parts = parts.iter().map(|t| t.to_string()).collect();
        p.act(&Action::ImportAssets(ImportAssets { package: package.into(), parts })).map(drop)
    };
    for parts in [&["layer:left"][..], &["sound:x"], &["glow"]] {
        assert!(import(&mut project, &wpak, parts).is_err(), "{parts:?}");
    }
    let scene = root.path().join("scene.wpak");
    package::build_saved(&project.dir, &project.doc, &scene, None).unwrap();
    let error = import(&mut project, &scene, &[]).unwrap_err().to_string();
    assert!(error.contains("is not an assets package"), "{error}");
    assert!(project.history.done.is_empty());
    let from = Project::open(&root.path().join("source")).unwrap();
    let offer = Offer { id: "Bad Id".into(), ..Offer::default() };
    let picks = [Pick::parse("layer:glow").unwrap()];
    assert!(make(&from.dir, &from.doc, &picks, &root.path().join("bad"), &offer).is_err());
    let offer = Offer { id: "org.example.more".into(), ..Offer::default() };
    assert!(make(&from.dir, &from.doc, &[], &root.path().join("none"), &offer).is_err());
    assert!(make(&from.dir, &from.doc, &picks, &root.path().join("parts"), &offer).is_err());
    let gone = [Pick::parse("layer:gone").unwrap()];
    assert!(make(&from.dir, &from.doc, &gone, &root.path().join("gone"), &offer).is_err());
}
