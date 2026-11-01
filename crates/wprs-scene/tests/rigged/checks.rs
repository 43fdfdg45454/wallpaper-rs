use super::kit::{arm, fox, scene};
use serde_json::json;

fn issue(extra: serde_json::Value) -> String {
    scene(fox(extra), arm().bytes()).map(|_| String::new()).unwrap_or_else(|e| e.to_string())
}

#[test]
fn clip_tracks_name_bones_shapes_and_parts_of_the_file() {
    let ok = json!({ "clips": [{ "id": "c", "bones": { "arm": { "rotation": [[0, 10], [1, 20]], "position": [[0, [1, 2]]] } },
        "shapes": { "lift": [[0, 0.5]] }, "order": { "patch": [[0, 3]] } }], "animations": [{ "clip": "c" }] });
    assert_eq!(issue(ok), "");
    for (bad, said) in [
        (json!({ "bones": { "tail": { "rotation": [[0, 1]] } } }), "tail"),
        (json!({ "shapes": { "frown": [[0, 1]] } }), "frown"),
        (json!({ "order": { "leg": [[0, 1]] } }), "leg"),
        (json!({ "bones": { "arm": { "rotation": [[1, 0], [0, 1]] } } }), "rotation"),
        (json!({ "shapes": { "lift": [[0, 2]] } }), "2"),
        (json!({ "order": { "patch": [[0, 5000]] } }), "5000"),
        (json!({ "bones": { "arm": { "scale": [[0, [1e9, 1]]] } } }), "scale"),
    ] {
        let mut clip = json!({ "id": "c" });
        bad.as_object().unwrap().iter().for_each(|(k, v)| clip[k] = v.clone());
        let found = issue(json!({ "clips": [clip] }));
        assert!(found.contains(said), "{said}: {found}");
    }
}

#[test]
fn rules_may_not_close_a_loop_of_parents() {
    assert_eq!(issue(json!({ "rules": [{ "bone": "hand", "parent": "root", "weight": 1 }] })), "");
    let found = issue(json!({ "rules": [{ "bone": "root", "parent": "hand", "weight": 1 }] }));
    assert!(found.contains("rules/0/parent"), "{found}");
    let both = json!({ "rules": [{ "bone": "patch", "parent": "hand", "weight": 1 }, { "bone": "hand", "parent": "patch", "weight": 1 }] });
    assert!(issue(both).contains("rules/"));
}

#[test]
fn a_mesh_that_cannot_be_read_names_its_asset() {
    let mut gl = arm();
    gl.doc["nodes"][3]["children"] = json!([1]);
    let found = scene(fox(json!({})), gl.bytes()).unwrap_err().to_string();
    assert!(found.contains("/assets/arm") && found.contains("cycle"), "{found}");
}
