use crate::{Cut, Runtime, SCHEMA, Scene, Sketch, SketchBone, Values};
use serde_json::{Value, json};
use std::sync::Arc;

fn reaching(target: Value) -> Runtime {
    let mut s = Sketch::cut(&vec![255; 64 * 64], [64, 64], Cut::Grid, 4);
    let bone = |name: &str, parent, at| SketchBone { name: name.into(), parent, at };
    s.bones = vec![
        bone("root", None, [4.0, 32.0]),
        bone("arm", Some(0), [24.0, 32.0]),
        bone("hand", Some(1), [44.0, 32.0]),
    ];
    s.bind();
    let glb = s.glb();
    let ik = json!({ "chain": ["root", "arm", "hand"], "target": target,
        "limits": { "arm": [-150, -5], "hand": [-150, -5] } });
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] },
        "assets": { "mesh": { "path": "mesh.glb" } },
        "layers": [{ "id": "fox", "type": "mesh", "mesh": "mesh", "ik": [ik] }] });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&Default::default(), move |_: &str, _: usize| Ok::<_, String>(glb.clone())).unwrap();
    Runtime::new(Arc::new(scene), Values::new())
}

#[test]
fn a_chain_whose_pose_and_target_stay_still_runs_no_rounds_after_the_first_frame() {
    let mut rt = reaching(json!({ "keys": [[0, [20, 20]], [0.04, [30, 50]]] }));
    (0..3).for_each(|_| rt.advance(1.0 / 60.0));
    let first = rt.puppets[0].chains[0].clone();
    assert!(first.ran > 0 && first.turns.iter().any(|t| t.abs() > 0.1), "{first:?}");
    for _ in 0..5 {
        rt.advance(1.0 / 60.0);
        let now = &rt.puppets[0].chains[0];
        assert_eq!((now.ran, &now.turns), (0, &first.turns));
    }
}

#[test]
fn a_moving_target_is_solved_again_on_every_frame() {
    let mut rt = reaching(json!({ "keys": [[0, [30, 50]], [1, [20, 20]]] }));
    for _ in 0..5 {
        rt.advance(1.0 / 60.0);
        assert!(rt.puppets[0].chains[0].ran > 0);
    }
}
