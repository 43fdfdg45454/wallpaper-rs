use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::inspect::fields;
use wprs_editor::starters::starter;

fn project(root: &TempDir) -> Project {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
    let made = starter("empty", "org.example.tune", "Tune", "en").unwrap();
    let mut p = Project::create(&root.path().join("tune"), made).unwrap();
    act(&mut p, json!({ "add_layer": { "kind": "text", "id": "title" } })).unwrap();
    p
}

fn act(p: &mut Project, action: Value) -> anyhow::Result<()> {
    p.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

#[test]
fn a_text_follows_the_media_player_and_says_so_in_the_inspector() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/0/text", "value": { "media": "artist" } } }))
        .unwrap();
    let shown = fields(&p.doc, "/scene/layers/0");
    let text = shown.iter().find(|f| f.key == "text").unwrap();
    assert_eq!((text.source.as_str(), text.bindable), ("media", true));
    let problems = wprs_editor::package::problems(&p.dir, &p.doc);
    assert!(problems.iter().all(|c| !c.pointer.contains("/text")), "{problems:?}");
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_ne!(fields(&p.doc, "/scene/layers/0").iter().find(|f| f.key == "text").unwrap().source, "media");
}

#[test]
fn the_preview_carries_a_made_up_track_with_its_quality() {
    let look = json!({ "quality": "q2", "media": { "title": "Night Drive", "playing": true } });
    let staged = wprs_editor::rigs::staged(&look).unwrap();
    assert_eq!(staged["media"]["title"], "Night Drive");
    assert_eq!(staged["quality"], "q2");
    let only = wprs_editor::rigs::staged(&json!({ "media": { "title": "x" } })).unwrap();
    assert!(only["quality"].is_null() && only["media"].is_object());
    assert!(wprs_editor::rigs::staged(&json!({ "media": "x" })).is_none());
}
