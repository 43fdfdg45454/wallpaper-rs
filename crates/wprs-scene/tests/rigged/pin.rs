use super::kit::{at, close, layers, runtime};
use serde_json::json;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 64.0, 64.0];

fn pinned(attach: serde_json::Value) -> Runtime {
    let mut rt = layers(json!([
        { "id": "fox", "type": "mesh", "mesh": "arm", "size": [64, 64],
          "clips": [{ "id": "wave", "animation": "wave", "loop": "once" }], "animations": [{ "clip": "wave" }] },
        { "id": "badge", "type": "solid", "color": "#ffffff", "size": [4, 4], "attach": attach }
    ]));
    rt.advance(2.0);
    rt
}

fn world(rt: &Runtime, id: &str) -> [f32; 6] {
    rt.frame(VIEW, Quality::Q3).draws.iter().find(|d| d.layer.id == id).unwrap().world.0
}

#[test]
fn a_layer_follows_a_bone_with_its_turn_and_offset() {
    let rt = pinned(json!({ "layer": "fox", "bone": "hand" }));
    let w = world(&rt, "badge");
    assert!(close([w[4], w[5]], [32.0, 8.0], 1e-3), "{w:?}");
    assert!(close([w[0], w[1]], [0.0, -1.0], 1e-4), "the badge turned with the arm: {w:?}");
    let rt = pinned(json!({ "layer": "fox", "bone": "hand", "offset": [10, 0] }));
    let w = world(&rt, "badge");
    assert!(close([w[4], w[5]], [32.0, -2.0], 1e-3), "the offset turns with the bone: {w:?}");
    let rt = pinned(json!({ "layer": "fox", "bone": "hand", "inherit": ["position"] }));
    let w = world(&rt, "badge");
    assert!(close([w[4], w[5]], [32.0, 8.0], 1e-3) && close([w[0], w[1]], [1.0, 0.0], 1e-4), "{w:?}");
}

#[test]
fn a_layer_follows_a_point_of_the_file_and_hides_with_its_mesh() {
    let rt = pinned(json!({ "layer": "fox", "point": "patch" }));
    let w = world(&rt, "badge");
    assert!(close([w[4], w[5]], [32.0, 32.0], 1e-3), "{w:?}");
    let mut rt = layers(json!([
        { "id": "fox", "type": "mesh", "mesh": "arm", "visible": false },
        { "id": "badge", "type": "solid", "color": "#ffffff", "size": [4, 4], "attach": { "layer": "fox", "bone": "hand" } }
    ]));
    rt.advance(0.1);
    assert!(rt.frame(VIEW, Quality::Q3).draws.iter().all(|d| d.layer.id != "badge" || d.hidden));
}

#[test]
fn scripts_turn_move_and_scale_bones_for_the_next_frame() {
    let mut rt = runtime(json!({}));
    let set = |rt: &mut Runtime, rigs: serde_json::Value| {
        rt.set_script(serde_json::from_value(json!({ "rigs": rigs })).unwrap());
        rt.advance(1.0 / 60.0);
    };
    set(&mut rt, json!({ "fox": { "bones": { "arm": { "rotation": -90 } } } }));
    assert!(close(at(&rt, "hand"), [0.0, 24.0], 1e-3), "{:?}", at(&rt, "hand"));
    set(&mut rt, json!({ "fox": { "bones": { "hand": { "position": [40, 32], "world": true } } } }));
    assert!(close(at(&rt, "hand"), [8.0, 0.0], 1e-3), "{:?}", at(&rt, "hand"));
    set(&mut rt, json!({ "fox": { "bones": { "root": { "rotation": 90, "world": true } } } }));
    assert!(close(at(&rt, "hand"), [-24.0, -48.0], 1e-3), "{:?}", at(&rt, "hand"));
    set(&mut rt, json!({ "fox": { "bones": { "arm": { "position": [0, 10] } } } }));
    assert!(close(at(&rt, "arm"), [0.0, -10.0], 1e-3), "{:?}", at(&rt, "arm"));
    set(&mut rt, json!({}));
    assert!(close(at(&rt, "hand"), [24.0, 0.0], 1e-4), "letting go returns to the pose");
    set(&mut rt, json!({ "fox": { "shapes": { "lift": 1 } } }));
    let morphs = &rt.puppet("fox").unwrap().posed().morphs;
    assert!(morphs.iter().flatten().any(|s| s.1 == 1.0), "{morphs:?}");
}

#[test]
fn a_touch_names_the_hit_bone_under_it() {
    let rt = runtime(json!({ "hit": ["arm"] }));
    let p = rt.puppet("fox").unwrap();
    let Kind::Mesh(m) = &rt.scene.layer("fox").unwrap().kind else { panic!() };
    assert_eq!(p.bone_at(m, [12.0, 0.0]), Some("arm"));
    assert_eq!(p.bone_at(m, [-12.0, 0.0]), None, "the root is not listed");
    assert_eq!(p.bone_at(m, [12.0, 20.0]), None, "nothing is drawn there");
    let rt = runtime(json!({ "hit": ["root"] }));
    let Kind::Mesh(m) = &rt.scene.layer("fox").unwrap().kind else { panic!() };
    assert_eq!(
        rt.puppet("fox").unwrap().bone_at(m, [12.0, 0.0]),
        Some("root"),
        "a child counts for its parent"
    );
    let mut doc = serde_json::to_value(&*rt.scene).unwrap();
    doc["scripts"] = json!([{ "path": "a.js" }]);
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&Default::default(), |_: &str, _: usize| Ok::<_, String>(super::kit::arm().bytes())).unwrap();
    let mut rt = Runtime::new(std::sync::Arc::new(scene), Values::new());
    let full = Hitbox {
        layer: "fox".into(),
        to_box: Affine::scale([1.0 / 64.0; 2]),
        to_local: Affine::IDENTITY,
        cell: [0.0, 0.0, 1.0, 1.0],
        alpha: 1.0,
        threshold: None,
        coverage: None,
        unlean: None,
    };
    let press = Press { at: Some([44.0 / 64.0, 0.5]), buttons: 1 };
    rt.set_pointer(PointerState { pos: press.at, buttons: 1, inside: true, ..PointerState::default() });
    let mut denied = Runtime::new(rt.scene.clone(), Values::new());
    rt.point(&Hitboxes(vec![full.clone()]), VIEW, &[press], (true, true));
    let touches = rt.take_touches();
    assert!(
        touches.iter().any(|t| t.kind == TouchKind::Down && t.bone.as_deref() == Some("root")),
        "{touches:?}"
    );
    denied.point(&Hitboxes(vec![full]), VIEW, &[press], (false, false));
    assert!(denied.take_touches().is_empty(), "without the pointer permission no bone is told");
}
