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

#[test]
fn chains_bodies_and_attachments_are_checked_against_the_file() {
    let fine = json!({ "ik": [{ "chain": ["root", "arm"], "target": "bone:patch", "limits": { "arm": [-45, 45] } }],
        "physics": [{ "bone": "hand", "mode": "rope", "rotation": [-30, 30], "translation": [-4, 4] }], "hit": ["arm"] });
    assert_eq!(issue(fine), "");
    for (bad, said) in [
        (json!({ "ik": [{ "chain": ["root", "hand"], "target": [1, 1] }] }), "hang from the one before"),
        (json!({ "ik": [{ "chain": ["arm", "root"], "target": [1, 1] }] }), "hang from the one before"),
        (json!({ "ik": [{ "chain": ["root", "leg"], "target": [1, 1] }] }), "leg"),
        (json!({ "ik": [{ "chain": ["root", "arm"], "target": "bone:tail" }] }), "tail"),
        (json!({ "ik": [{ "chain": ["root", "arm"], "target": [1, 1], "weight": 3 }] }), "weight"),
        (
            json!({ "ik": (0..17).map(|_| json!({ "chain": ["root", "arm"], "target": [1, 1] })).collect::<Vec<_>>() }),
            "at most 16",
        ),
        (json!({ "physics": [{ "bone": "tail" }] }), "tail"),
        (json!({ "physics": [{ "bone": "arm" }, { "bone": "arm" }] }), "physics/1/bone"),
        (json!({ "physics": [{ "bone": "arm", "translation": [5, -5] }] }), "translation/1"),
        (json!({ "physics": [{ "bone": "arm", "gravity": 11 }] }), "gravity"),
        (json!({ "physics": (0..65).map(|_| json!({ "bone": "arm" })).collect::<Vec<_>>() }), "at most 64"),
        (json!({ "hit": ["arm", "arm"] }), "hit"),
        (json!({ "hit": ["tail"] }), "tail"),
    ] {
        let found = issue(bad.clone());
        assert!(found.contains(said), "{bad}: {said}: {found}");
    }
}

#[test]
fn attachments_name_bones_and_points_of_meshes_without_loops() {
    let two = |attach: serde_json::Value, back: Option<serde_json::Value>| {
        let mut fox = fox(json!({}));
        if let Some(b) = back {
            fox["attach"] = b;
        }
        let layers = json!([fox, { "id": "badge", "type": "mesh", "mesh": "arm", "attach": attach }]);
        let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 64] },
            "assets": { "arm": { "path": "arm.glb" } }, "layers": layers });
        let mut scene = wprs_scene::Scene::from_json(&text.to_string()).map_err(|e| e.to_string())?;
        scene
            .adopt(&Default::default(), |_: &str, _: usize| Ok::<_, String>(arm().bytes()))
            .map_err(|e| e.to_string())
    };
    assert_eq!(two(json!({ "layer": "fox", "bone": "hand" }), None), Ok(()));
    assert_eq!(two(json!({ "layer": "fox", "point": "patch" }), None), Ok(()));
    let missing = two(json!({ "layer": "fox", "bone": "tail" }), None).unwrap_err();
    assert!(missing.contains("attach/bone") && missing.contains("tail"), "{missing}");
    let both = two(json!({ "layer": "fox", "bone": "hand", "point": "patch" }), None).unwrap_err();
    assert!(both.contains("/attach"), "{both}");
    let looped =
        two(json!({ "layer": "fox", "bone": "hand" }), Some(json!({ "layer": "badge", "bone": "hand" })));
    assert!(looped.unwrap_err().contains("depends on itself"));
}
