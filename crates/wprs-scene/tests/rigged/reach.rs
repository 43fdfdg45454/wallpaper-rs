use super::kit::{at, close, layers, runtime};
use serde_json::json;
use wprs_scene::{PointerState, Uses};

fn reaching(target: serde_json::Value, more: serde_json::Value) -> wprs_scene::Runtime {
    let mut ik = json!({ "chain": ["root", "arm"], "target": target });
    more.as_object().unwrap().iter().for_each(|(k, v)| ik[k] = v.clone());
    let mut rt = runtime(json!({ "ik": [ik] }));
    rt.advance(1.0 / 60.0);
    rt
}

#[test]
fn a_two_bone_chain_reaches_a_point_and_bends_the_way_it_is_told() {
    let rt = reaching(json!([32, 8]), json!({}));
    assert!(close(at(&rt, "hand"), [0.0, 24.0], 0.01), "{:?}", at(&rt, "hand"));
    assert!(close(at(&rt, "arm"), [-24.0, 24.0], 0.01), "{:?}", at(&rt, "arm"));
    let rt = reaching(json!([32, 8]), json!({ "bend": "counterclockwise" }));
    assert!(close(at(&rt, "hand"), [0.0, 24.0], 0.01));
    assert!(close(at(&rt, "arm"), [0.0, 0.0], 0.01), "{:?}", at(&rt, "arm"));
}

#[test]
fn a_chain_stretches_towards_a_point_out_of_reach() {
    let rt = reaching(json!([232, 32]), json!({}));
    assert!(close(at(&rt, "hand"), [24.0, 0.0], 0.01), "{:?}", at(&rt, "hand"));
    let rt = reaching(json!([8, 232]), json!({}));
    assert!(close(at(&rt, "hand"), [-24.0, -48.0], 0.01), "{:?}", at(&rt, "hand"));
}

#[test]
fn limits_hold_each_bone_within_its_range() {
    let rt = reaching(json!([2, 12]), json!({ "limits": { "root": [-10, 10] } }));
    let turned = 10f32.to_radians();
    assert!(
        close(at(&rt, "arm"), [-24.0 + 24.0 * turned.cos(), 24.0 * turned.sin()], 0.01),
        "{:?}",
        at(&rt, "arm")
    );
    let rt = reaching(json!([32, 8]), json!({ "limits": { "arm": [0, 0] } }));
    assert!(close(at(&rt, "hand"), [-24.0 + 48.0 * 0.5f32.sqrt(), 48.0 * 0.5f32.sqrt()], 0.01));
}

#[test]
fn a_weight_blends_the_reach_with_the_pose() {
    let rt = reaching(json!([32, 8]), json!({ "weight": 0 }));
    assert!(close(at(&rt, "hand"), [24.0, 0.0], 1e-4));
    let rt = reaching(json!([32, 8]), json!({ "weight": 0.5, "limits": { "arm": [0, 0] } }));
    let half = 22.5f32.to_radians();
    assert!(
        close(at(&rt, "hand"), [-24.0 + 48.0 * half.cos(), 48.0 * half.sin()], 0.01),
        "{:?}",
        at(&rt, "hand")
    );
}

#[test]
fn a_longer_chain_reaches_with_iterations() {
    let mut rt = runtime(json!({ "ik": [{ "chain": ["root", "arm", "hand"], "target": [32, -8] }] }));
    rt.advance(1.0 / 60.0);
    let p = rt.puppet("fox").unwrap();
    let f = p.bone_frames([64.0, 64.0])[p.rig.joint("hand").unwrap()];
    let tip = [f[0] * 24.0 + f[4] - 32.0, 32.0 - (f[1] * 24.0 + f[5])];
    assert!(close(tip, [0.0, 40.0], 0.05), "{tip:?}");
}

#[test]
fn chains_reach_for_bones_and_layers() {
    let rt = reaching(json!("bone:patch"), json!({}));
    assert!(close(at(&rt, "hand"), [0.0, 0.0], 0.01), "{:?}", at(&rt, "hand"));
    let mut rt = layers(json!([
        { "id": "fox", "type": "mesh", "mesh": "arm", "size": [64, 64],
          "ik": [{ "chain": ["root", "arm"], "target": "layer:dot" }] },
        { "id": "dot", "type": "solid", "color": "#ffffff", "size": [2, 2], "transform": { "position": [32, 8] } }
    ]));
    rt.advance(1.0 / 60.0);
    assert!(close(at(&rt, "hand"), [0.0, 24.0], 0.01), "{:?}", at(&rt, "hand"));
}

#[test]
fn the_pointer_moves_a_chain_only_while_the_scene_may_follow_it() {
    let pointer = PointerState { pos: Some([0.5, 0.125]), inside: true, ..PointerState::default() };
    let mut rt = runtime(json!({ "ik": [{ "chain": ["root", "arm"], "target": "cursor" }] }));
    assert!(rt.uses().cursor);
    assert!(rt.set_pointer(pointer));
    rt.advance(1.0 / 60.0);
    assert!(close(at(&rt, "hand"), [0.0, 24.0], 0.01), "{:?}", at(&rt, "hand"));
    let mut rt = runtime(json!({ "ik": [{ "chain": ["root", "arm"], "target": "cursor" }] }));
    rt.declare(Uses { cursor: false, ..Uses::default() });
    rt.set_pointer(pointer);
    rt.advance(1.0 / 60.0);
    assert!(close(at(&rt, "hand"), [24.0, 0.0], 1e-4), "{:?}", at(&rt, "hand"));
}
