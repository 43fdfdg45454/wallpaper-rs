use super::kit::{act, rigged};
use serde_json::json;
use wprs_editor::Project;

fn meshes(p: &Project) -> Vec<String> {
    let list = wprs_editor::assets::files(&p.dir).into_iter().map(|(f, _)| f);
    list.filter(|f| f.starts_with("meshes/")).collect()
}

fn used(p: &Project) -> String {
    p.doc["scene"]["assets"]["net"]["path"].as_str().unwrap().to_owned()
}

#[test]
fn old_meshes_wait_outside_the_folder_for_undo_and_stay_out_of_the_package() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    (0..2).for_each(|_| act(&mut p, json!({ "undo": {} })).unwrap());
    let (first, kept) = (used(&p), std::fs::read(p.dir.join(used(&p))).unwrap());
    (0..2).for_each(|_| act(&mut p, json!({ "redo": {} })).unwrap());
    for x in [0.3, 0.4, 0.6, 0.7] {
        act(&mut p, json!({ "bone": { "layer": "net", "name": "arm", "at": [x, 0.5] } })).unwrap();
    }
    assert!(meshes(&p).len() > 4);
    p.save().unwrap();
    assert_eq!(meshes(&p), [used(&p)]);
    act(&mut p, json!({ "undo": {} })).unwrap();
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert!(p.dir.join(used(&p)).is_file(), "undo brings back {}", used(&p));
    p.save().unwrap();
    let back = std::fs::read(p.dir.join(used(&p))).unwrap();
    act(&mut p, json!({ "bone": { "layer": "net", "name": "arm", "at": [0.8, 0.5] } })).unwrap();
    p.save().unwrap();
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert!(std::fs::read(p.dir.join(used(&p))).unwrap() == back, "undo finds the mesh it named");
    act(&mut p, json!({ "redo": {} })).unwrap();
    p.save().unwrap();
    act(&mut p, json!({ "undo": {} })).unwrap();
    p.save().unwrap();
    let mut reopened = Project::open(&p.dir).unwrap();
    act(&mut reopened, json!({ "redo": {} })).unwrap();
    assert!(reopened.dir.join(used(&reopened)).is_file());
    reopened.save().unwrap();
    assert_eq!(meshes(&reopened), [used(&reopened)]);
    std::fs::write(reopened.dir.join("meshes/left.glb"), b"old").unwrap();
    assert_eq!(wprs_editor::assets::prune(&reopened.dir, &reopened.doc).unwrap(), ["meshes/left.glb"]);
    let out = root.path().join("fox.wpak");
    wprs_editor::package::build(&mut reopened, &out, None).unwrap();
    let package = wprs_format::open(&out, &wprs_format::Limits::default()).unwrap();
    let packed: Vec<&str> =
        package.entries.iter().map(|e| e.path.as_str()).filter(|f| f.starts_with("meshes/")).collect();
    assert_eq!(packed, [used(&reopened)]);
    for _ in 0..20 {
        if used(&reopened) == first {
            break;
        }
        act(&mut reopened, json!({ "undo": {} })).unwrap();
        reopened.save().unwrap();
    }
    assert_eq!(used(&reopened), first);
    let back = std::fs::read(reopened.dir.join(first)).unwrap() == kept;
    assert!(back, "a new mesh never takes the name of a kept one");
}
