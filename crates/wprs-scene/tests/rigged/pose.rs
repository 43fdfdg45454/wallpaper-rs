use super::kit::{moved, near, runtime};
use serde_json::json;

const VIEW: [f32; 4] = [0.0, 0.0, 64.0, 64.0];

#[test]
fn the_rest_pose_leaves_every_vertex_in_place() {
    let rt = runtime(json!({}));
    for (bone, p) in
        [(0, [-12.0, 4.0, 0.0]), (1, [12.0, -4.0, 0.0]), (2, [24.0, 0.0, 0.0]), (3, [20.0, 2.0, 0.0])]
    {
        assert!(near(moved(&rt, bone, p), p), "bone {bone}");
    }
}

#[test]
fn a_bent_bone_turns_its_vertices_and_its_children_clockwise() {
    let rt = runtime(json!({ "clips": [{ "id": "bend", "bones": { "arm": { "rotation": [[0, 90]] } } }],
        "animations": [{ "clip": "bend" }] }));
    assert!(near(moved(&rt, 1, [12.0, 0.0, 0.0]), [0.0, -12.0, 0.0]));
    assert!(near(moved(&rt, 2, [24.0, 0.0, 0.0]), [0.0, -24.0, 0.0]));
    assert!(near(moved(&rt, 0, [-12.0, 0.0, 0.0]), [-12.0, 0.0, 0.0]));
}

#[test]
fn a_clip_of_the_file_plays_its_keys_at_a_given_time() {
    let mut rt = runtime(
        json!({ "clips": [{ "id": "wave", "animation": "wave" }], "animations": [{ "clip": "wave" }] }),
    );
    rt.seek(0.5);
    let h = 12.0 * std::f32::consts::FRAC_1_SQRT_2;
    assert!(near(moved(&rt, 1, [12.0, 0.0, 0.0]), [h, h, 0.0]));
    let morphs = &rt.puppet("fox").unwrap().posed().morphs[0];
    assert!((morphs[0].1 - 0.5).abs() < 1e-4 && morphs[1].1 == 0.0, "{morphs:?}");
}

#[test]
fn layers_fade_in_and_out_with_their_blend_times() {
    let mut rt = runtime(json!({ "clips": [{ "id": "bend", "bones": { "arm": { "rotation": [[0, 90]] } } }],
        "animations": [{ "clip": "bend", "blend_in": 1.0, "blend_out": 1.0 }] }));
    assert!(near(moved(&rt, 1, [12.0, 0.0, 0.0]), [12.0, 0.0, 0.0]));
    rt.advance(0.5);
    let h = 12.0 * std::f32::consts::FRAC_1_SQRT_2;
    assert!(near(moved(&rt, 1, [12.0, 0.0, 0.0]), [h, -h, 0.0]));
    rt.advance(0.5);
    assert!(near(moved(&rt, 1, [12.0, 0.0, 0.0]), [0.0, -12.0, 0.0]));
    let stop = serde_json::from_value(json!({ "videos": { "fox#bend": { "commands": ["stop"] } } })).unwrap();
    rt.set_script(stop);
    rt.advance(0.5);
    assert!(near(moved(&rt, 1, [12.0, 0.0, 0.0]), [h, -h, 0.0]));
    rt.advance(0.6);
    assert!(near(moved(&rt, 1, [12.0, 0.0, 0.0]), [12.0, 0.0, 0.0]));
}

#[test]
fn additive_layers_add_and_masked_layers_move_only_their_bones() {
    let bend = |id: &str, deg: f32| json!({ "id": id, "bones": { "arm": { "rotation": [[0, deg]] }, "hand": { "rotation": [[0, deg]] } } });
    let rt = runtime(json!({ "clips": [bend("a", 60.0), bend("b", 30.0)],
        "animations": [{ "clip": "a", "bones": ["arm"] }, { "clip": "b", "additive": true, "bones": ["arm"] }] }));
    assert!(near(moved(&rt, 1, [12.0, 0.0, 0.0]), [0.0, -12.0, 0.0]));
    assert!(near(moved(&rt, 2, [36.0, 0.0, 0.0]), [0.0, -36.0, 0.0]), "the hand only follows the arm");
    let half = runtime(json!({ "clips": [bend("a", 90.0)], "animations": [{ "clip": "a", "weight": 0.5 }] }));
    let h = 12.0 * std::f32::consts::FRAC_1_SQRT_2;
    assert!(near(moved(&half, 1, [12.0, 0.0, 0.0]), [h, -h, 0.0]));
}

#[test]
fn a_rule_hands_a_bone_to_another_parent_keeping_its_pose() {
    let wave = json!({ "clips": [{ "id": "wave", "animation": "wave", "loop": "once" }], "animations": [{ "clip": "wave" }] });
    let mut free = runtime(wave.clone());
    free.seek(1.0);
    assert!(near(moved(&free, 2, [24.0, 0.0, 0.0]), [0.0, 24.0, 0.0]));
    let mut ruled = wave;
    ruled["rules"] = json!([{ "bone": "hand", "parent": "root", "weight": 1.0 }]);
    let mut rt = runtime(ruled);
    rt.seek(1.0);
    assert!(near(moved(&rt, 2, [24.0, 0.0, 0.0]), [24.0, 0.0, 0.0]));
}

#[test]
fn shapes_and_expressions_weigh_the_blend_shapes() {
    let rt = runtime(
        json!({ "shapes": { "lift": 0.25, "smile": 1.0 }, "expressions": { "smile": { "lift": 0.5 } } }),
    );
    let morphs = rt.puppet("fox").unwrap().posed().morphs[0];
    assert_eq!(morphs[0], (0, 0.75));
    let none = runtime(json!({}));
    assert!(none.puppet("fox").unwrap().posed().morphs[0].iter().all(|m| m.1 == 0.0));
}

#[test]
fn parts_follow_their_order_visibility_and_cutouts() {
    let order = |rt: &wprs_scene::Runtime| {
        rt.puppet("fox").unwrap().posed().draws.iter().map(|d| d.part).collect::<Vec<_>>()
    };
    assert_eq!(order(&runtime(json!({}))), [0, 1]);
    assert_eq!(order(&runtime(json!({ "parts": { "body": { "order": 2 } } }))), [1, 0]);
    let swap = json!({ "clips": [{ "id": "swap", "loop": "once", "order": { "body": [[0, 3, "step"], [1, -1]] } }],
        "animations": [{ "clip": "swap" }] });
    let mut rt = runtime(swap);
    assert_eq!(order(&rt), [1, 0]);
    rt.seek(1.5);
    assert_eq!(order(&rt), [0, 1]);
    assert_eq!(order(&runtime(json!({ "parts": { "patch": { "visible": false } } }))), [0]);
    let cut = runtime(json!({ "parts": { "patch": { "clip_to": "body" } } }));
    assert_eq!(cut.puppet("fox").unwrap().posed().draws[1].clip, Some(0));
    let frame = cut.frame(VIEW, wprs_scene::Quality::Q3);
    assert!(matches!(frame.draws[0].content, wprs_scene::Content::Mesh { pose: 1, .. }));
}
