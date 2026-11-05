use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

mod effects;

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

pub fn act(project: &mut Project, action: Value) -> anyhow::Result<()> {
    let action = serde_json::from_value::<Action>(action).unwrap();
    assert!(wprs_editor::ACTIONS.contains(&action.name().as_str()), "{}", action.name());
    project.act(&action).map(drop)
}

pub fn refused(project: &mut Project, action: Value) -> String {
    let before = project.doc.clone();
    let error = act(project, action).unwrap_err().to_string();
    assert_eq!(project.doc, before, "{error}");
    error
}

pub fn project(root: &TempDir) -> Project {
    state();
    let dir = root.path().join("glow");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.glow", "Glow", "en").unwrap()).unwrap();
    act(&mut project, json!({"add_layer": {"kind": "solid", "id": "card"}})).unwrap();
    project
}

pub fn at(project: &Project, pointer: &str) -> Value {
    project.doc.pointer(pointer).cloned().unwrap_or(Value::Null)
}
