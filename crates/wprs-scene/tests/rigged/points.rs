use super::kit::{arm, fox, scene};
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_scene::{Runtime, Values};

fn posed(layer: Value, point: bool) -> Vec<[f32; 6]> {
    let mut gl = arm();
    if point {
        gl.doc["nodes"][3]["children"] = json!([5]);
        let nodes = gl.doc["nodes"].as_array_mut().unwrap();
        nodes.push(json!({ "name": "badge", "translation": [5.0, 10.0, 0.0], "extras": { "point": true } }));
    }
    let mut rt = Runtime::new(Arc::new(scene(fox(layer), gl.bytes()).unwrap()), Values::new());
    (0..30).for_each(|_| rt.advance(1.0 / 60.0));
    rt.puppet("fox").unwrap().bone_frames([64.0, 64.0])[..4].to_vec()
}

fn same(a: &[[f32; 6]], b: &[[f32; 6]]) -> bool {
    a.iter().flatten().zip(b.iter().flatten()).all(|(x, y)| (x - y).abs() < 1e-4)
}

#[test]
fn a_point_never_counts_as_the_end_of_its_bone() {
    let chain = json!({ "ik": [{ "chain": ["arm", "hand"], "target": [80, 32] }] });
    assert!(same(&posed(chain.clone(), true), &posed(chain.clone(), false)));
    assert!(
        same(&posed(chain, true), &posed(json!({}), false)),
        "a chain aimed at its own end stays at rest"
    );
    let spring = json!({ "physics": [{ "bone": "hand", "gravity": 5, "stiffness": 0.2 }] });
    assert!(same(&posed(spring.clone(), true), &posed(spring.clone(), false)));
    let rope = json!({ "physics": [{ "bone": "arm", "mode": "rope", "gravity": 3 }] });
    assert!(same(&posed(rope.clone(), true), &posed(rope, false)));
}
