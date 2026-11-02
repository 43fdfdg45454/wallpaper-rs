use serde_json::{Value, json};
use std::time::Duration;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::history::MERGE_MS;
use wprs_editor::starters::starter;

fn drag(project: &mut Project, action: Value) {
    project.drag(&serde_json::from_value::<Action>(action).unwrap()).unwrap();
    project.held();
}

#[test]
fn a_gesture_that_changes_nothing_never_joins_two_drags() {
    let root = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_STATE_HOME", root.path().join("state")) };
    let dir = root.path().join("dusk");
    let mut project =
        Project::create(&dir, starter("parallax", "org.example.dusk", "Dusk", "en").unwrap()).unwrap();
    let background = |value: &str| json!({"set": {"pointer": "/scene/canvas/background", "value": value}});
    drag(&mut project, background("#101010"));
    let steps = project.history.done.len();
    std::thread::sleep(Duration::from_millis(MERGE_MS + 100));
    let inputs = project.doc["manifest"]["inputs"].clone();
    assert!(inputs.is_array(), "the starter declares its inputs");
    drag(&mut project, json!({"set": {"pointer": "/manifest/inputs", "value": inputs}}));
    assert_eq!(project.history.done.len(), steps, "a gesture that changes nothing records nothing");
    drag(&mut project, background("#202020"));
    assert_eq!(project.history.done.len(), steps + 1);
}
