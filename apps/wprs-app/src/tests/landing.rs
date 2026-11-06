use crate::landing::{Settled, land, redo, settle};
use image::{ImageFormat, RgbImage};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

#[test]
fn a_scene_changed_during_an_import_is_answered_by_another_job_before_it_is_recorded() {
    state();
    let root = tempfile::tempdir().unwrap();
    let doc = starter("empty", "org.example.landing", "Landing", "en").unwrap();
    let mut project = Project::create(&root.path().join("scene"), doc).unwrap();
    let file = root.path().join("sky.png");
    RgbImage::from_pixel(8, 4, [40, 90, 160].into()).save_with_format(&file, ImageFormat::Png).unwrap();
    let import = Action::Import { file, id: Some("sky".into()), linear: false, atlas: None };
    let landed = land(&project.dir, project.doc.clone(), import).unwrap();
    let layer = Action::AddLayer {
        kind: "solid".into(),
        id: Some("tint".into()),
        name: None,
        source: None,
        into: None,
        at: None,
        file: None,
    };
    project.act(&layer).unwrap();
    let stale = match settle(&mut project, landed, "import").unwrap() {
        Settled::Stale(stale) => stale,
        Settled::Recorded => panic!("ops made for an older scene were recorded on the window thread"),
    };
    assert!(project.doc["scene"]["assets"].get("sky").is_none(), "the stale import was recorded");
    let again = redo(&project.dir, project.doc.clone(), stale).unwrap();
    assert!(matches!(settle(&mut project, again, "import").unwrap(), Settled::Recorded));
    assert!(project.doc["scene"]["assets"].get("sky").is_some(), "{}", project.doc["scene"]["assets"]);
    let layers = project.doc["scene"]["layers"].as_array().unwrap();
    assert!(layers.iter().any(|l| l["id"] == "tint"), "the edit made during the import was lost");
}
