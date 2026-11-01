use serde_json::{Value, json};
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

fn act(project: &mut Project, action: Value) -> anyhow::Result<()> {
    project.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

#[test]
fn masks_are_painted_per_channel_into_new_pictures_that_undo_restores() {
    let root = tempfile::tempdir().unwrap();
    state();
    let dir = root.path().join("veil");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.veil", "Veil", "en").unwrap()).unwrap();
    let layer = json!([{ "id": "fill", "type": "solid", "color": "#ffffff", "effects": [{ "effect": "blend_gradient" }] }]);
    act(&mut project, json!({ "set": { "pointer": "/scene/layers", "value": layer } })).unwrap();
    let pointer = "/scene/layers/0/effects/0/params/map";
    let stroke = |channel: Value, value: f32| {
        json!({ "paint_mask": { "pointer": pointer, "channel": channel, "radius": 0.1, "value": value,
            "points": [[0.4, 0.5], [0.6, 0.5]] } })
    };
    act(&mut project, stroke(json!(1), 1.0)).unwrap();
    let id = project.doc.pointer(pointer).and_then(Value::as_str).unwrap().to_owned();
    let path = |p: &Project| p.doc["scene"]["assets"][&id]["path"].as_str().unwrap().to_owned();
    let first = path(&project);
    assert_eq!(project.doc["scene"]["assets"][&id]["linear"], true);
    let picture = image::open(dir.join(&first)).unwrap().into_rgba8();
    let [r, g, b, a] = picture.get_pixel(256, 256).0;
    assert!(g == 255 && r == 0 && b == 0 && a == 255, "{:?}", picture.get_pixel(256, 256));
    assert_eq!(picture.get_pixel(10, 10).0, [0, 0, 0, 255]);
    act(&mut project, stroke(Value::Null, 0.0)).unwrap();
    let second = path(&project);
    let erased = image::open(dir.join(&second)).unwrap().into_rgba8();
    assert!(second != first && erased.get_pixel(256, 256).0 == [0, 0, 0, 255]);
    act(&mut project, json!({ "undo": {} })).unwrap();
    assert_eq!(path(&project), first);
    let wrong = json!({ "paint_mask": { "pointer": "/scene/canvas/background", "radius": 0.1, "points": [[0.5, 0.5]] } });
    assert!(act(&mut project, wrong).is_err());
    let far =
        json!({ "paint_mask": { "pointer": pointer, "channel": 5, "radius": 0.1, "points": [[0.5, 0.5]] } });
    assert!(act(&mut project, far).is_err());
    assert!(wprs_editor::package::problems(&project.dir, &project.doc).iter().all(|p| p.warning));
}
