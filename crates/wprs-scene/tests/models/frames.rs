use super::*;

#[test]
fn a_lone_model_gets_its_own_stage_and_a_still_one_asks_for_no_frames() {
    let s = scene(json!({}), json!([model("m", json!({ "position": [1, 0, 0], "rotation": [0, 90, 0] }))]));
    let rt = runtime(s);
    let frame = rt.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3);
    assert_eq!((frame.stages.len(), frame.placed.len()), (1, 1));
    let stage = &frame.stages[0];
    assert_eq!((stage.size, stage.slices, stage.eye.aspect), ([200.0, 100.0], 1, 2.0));
    let m = frame.placed[0].matrix;
    assert!((m[12] - 1.0).abs() < 1e-6 && m[0].abs() < 1e-6 && (m[2] + 1.0).abs() < 1e-6, "{m:?}");
    let Content::Model { key, stage: s, .. } = frame.draws[0].content else { panic!("no model draw") };
    assert_eq!(s, 0);
    assert_ne!(key, 0);
    assert_eq!(frame.wake, Wake::Idle);
    let again = rt.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3);
    assert_eq!(again.stages[0].key, key);
}

#[test]
fn a_moving_camera_changes_the_key_and_keeps_waking() {
    let cam = json!({ "camera3d": { "fov": { "keys": [[0, 40], [2, 80]] } } });
    let mut rt = runtime(scene(cam, json!([model("m", json!({}))])));
    let first = rt.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3).stages[0].key;
    rt.advance(0.5);
    let frame = rt.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3);
    assert_ne!(frame.stages[0].key, first);
    assert_eq!(frame.wake, Wake::Frame);
    assert!((frame.stages[0].eye.fov - 50.0).abs() < 0.1);
}

#[test]
fn a_3d_group_draws_its_models_as_slices_around_layers_that_test_depth() {
    let group = json!([{ "id": "world", "type": "group", "space": "3d", "layers": [
        { "id": "front", "type": "solid", "color": "#ffffff", "size": [10, 10] },
        model("a", json!({})),
        { "id": "near", "type": "solid", "color": "#ff0000", "size": [10, 10], "depth_test": true, "transform": { "z": 40 } },
        { "id": "far", "type": "solid", "color": "#00ff00", "size": [10, 10], "depth_test": true },
        model("b", json!({ "position": [0, 0, -3] })) ] }]);
    let rt = runtime(scene(json!({}), group));
    let frame = rt.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3);
    assert_eq!((frame.stages.len(), frame.stages[0].slices), (1, 3));
    let order: Vec<String> = frame
        .draws
        .iter()
        .map(|d| match d.content {
            Content::Model { slice, .. } if d.layer.id == "world" => format!("slice{slice}"),
            _ => d.layer.id.clone(),
        })
        .collect();
    assert_eq!(order, ["world", "slice0", "far", "slice1", "near", "slice2", "front", "a", "b"]);
    assert!(frame.draws.iter().filter(|d| ["a", "b"].contains(&d.layer.id.as_str())).all(|d| d.hidden));
    assert_eq!(frame.draws[0].end, frame.draws.len());
    let [near, far] = frame.stages[0].slice(1);
    assert!(near > 0.0 && far > near && (far - 5.0).abs() < 1e-3, "{near} {far}");
    let drawn = |id: &str| frame.placed.iter().find(|p| p.layer.id == id).map(|p| p.draw).unwrap();
    assert_eq!(frame.draws[drawn("b")].layer.id, "b");
}

#[test]
fn the_camera_projects_the_center_to_the_middle_with_reverse_depth() {
    let eye = Eye {
        at: [0.0, 0.0, 5.0],
        center: [0.0; 3],
        up: [0.0, 1.0, 0.0],
        fov: 60.0,
        near: 0.5,
        far: 50.0,
        aspect: 2.0,
    };
    let [x, y, z] = eye.project([0.0; 3], [200.0, 100.0]).unwrap();
    assert!((x - 100.0).abs() < 1e-3 && (y - 50.0).abs() < 1e-3 && z > 0.0 && z < 1.0);
    let near = eye.project([0.0, 0.0, 4.5], [200.0, 100.0]).unwrap()[2];
    let far = eye.project([0.0, 0.0, -45.0], [200.0, 100.0]).unwrap()[2];
    assert!((near - 1.0).abs() < 1e-4 && far.abs() < 1e-4, "{near} {far}");
    let p = eye.place([200.0, 50.0], 0.0, [200.0, 100.0]);
    let back = eye.project(p, [200.0, 100.0]).unwrap();
    assert!((back[0] - 200.0).abs() < 1e-2, "{back:?}");
    assert!(eye.project([0.0, 0.0, 9.0], [200.0, 100.0]).is_none());
}
