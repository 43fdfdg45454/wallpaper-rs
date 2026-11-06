mod web;

use serde_json::{Value, json};
use std::path::Path;
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

pub fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

pub fn page(root: &Path) -> Project {
    state();
    Project::create(&root.join("page"), starter("web", "org.example.page", "Page", "en").unwrap()).unwrap()
}

pub fn act(project: &mut Project, action: Value) -> anyhow::Result<()> {
    project.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

pub fn manifest(project: &mut Project, edit: Value) -> anyhow::Result<()> {
    act(project, json!({ "manifest": edit }))
}
