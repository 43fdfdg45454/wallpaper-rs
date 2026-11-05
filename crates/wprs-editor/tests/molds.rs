use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

fn act(p: &mut Project, action: Value) -> anyhow::Result<()> {
    p.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

fn project(root: &TempDir) -> Project {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
    let made = starter("empty", "org.example.molds", "Molds", "en").unwrap();
    Project::create(&root.path().join("molds"), made).unwrap()
}

#[test]
fn a_script_made_from_the_media_template_declares_every_media_event() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "import_script": { "file": "", "template": "media" } })).unwrap();
    act(&mut p, json!({ "import_script": { "file": "", "template": "media" } })).unwrap();
    let scripts = p.doc["scene"]["scripts"].clone();
    assert_eq!(scripts[0]["path"], "scripts/media.js");
    assert_eq!(scripts[1]["path"], "scripts/media-2.js");
    let events = ["mediaStatus", "mediaPlayback", "mediaProperties", "mediaThumbnail", "mediaTimeline"];
    assert_eq!(scripts[0]["events"], json!(events));
    assert_eq!(scripts[0]["uses"], json!(["media"]));
    assert!(p.dir.join("scripts/media.js").is_file());
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|c| c.warning), "{problems:?}");
    act(&mut p, json!({ "import_script": { "file": "plain.js", "template": "empty" } })).unwrap();
    assert_eq!(p.doc["scene"]["scripts"][2]["path"], "scripts/plain.js");
    let taken = act(&mut p, json!({ "import_script": { "file": "plain.js", "template": "empty" } }));
    assert!(taken.unwrap_err().to_string().contains("already exists"));
    let unknown = act(&mut p, json!({ "import_script": { "file": "x.js", "template": "nope" } }));
    assert!(unknown.unwrap_err().to_string().contains("no script template"));
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc["scene"]["scripts"].as_array().unwrap().len(), 2);
}
