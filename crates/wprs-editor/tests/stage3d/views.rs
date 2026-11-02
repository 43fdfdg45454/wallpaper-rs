use super::{ask, edit, layer, project, undone};
use serde_json::{Value, json};

#[test]
fn the_scene_camera_is_set_rolled_and_taken_from_the_view() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    edit(
        &mut p,
        json!({ "camera": { "fov": 40, "eye": [0, 2, 8], "center": [0, 1, 0], "near": 0.5, "far": 200 } }),
    )
    .unwrap();
    undone(&mut p, &before);
    let cam = &p.doc["scene"]["camera3d"];
    assert_eq!((cam["fov"].clone(), cam["eye"].clone()), (json!(40.0), json!([0.0, 2.0, 8.0])));
    edit(&mut p, json!({ "camera": { "roll": 90 } })).unwrap();
    let up = p.doc["scene"]["camera3d"]["up"].clone();
    assert!(up[1].as_f64().unwrap().abs() < 0.01 && up[0].as_f64().unwrap().abs() > 0.99, "{up}");
    for bad in [
        json!({ "fov": 0 }),
        json!({ "fov": 180 }),
        json!({ "far": 0.1 }),
        json!({ "up": [0, 0, 0] }),
        json!({ "roll": 400 }),
    ] {
        assert!(edit(&mut p, json!({ "camera": bad.clone() })).is_err(), "{bad}");
    }
    edit(&mut p, json!({ "view": { "orbit": [90, 0] } })).unwrap();
    edit(&mut p, json!({ "camera": { "from_view": true } })).unwrap();
    let eye = p.doc["scene"]["camera3d"]["eye"].clone();
    assert!(eye[0].as_f64().unwrap() > 7.0, "{eye}");
    edit(&mut p, json!({ "camera": { "clear": true } })).unwrap();
    assert!(p.doc["scene"].get("camera3d").is_none());
}

#[test]
fn the_view_orbits_pans_zooms_frames_and_looks_through_the_camera() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll", "at": [3, 0, 0] } } })).unwrap();
    let before = p.doc.clone();
    edit(&mut p, json!({ "view": { "orbit": [30, 20], "zoom": 2 } })).unwrap();
    undone(&mut p, &before);
    let v = p.doc["scene"]["ext"]["editor"]["view3d"].clone();
    assert_eq!(
        (v["yaw"].clone(), v["pitch"].clone(), v["distance"].clone()),
        (json!(30.0), json!(20.0), json!(2.5))
    );
    edit(&mut p, json!({ "view": { "pan": [100, 0] } })).unwrap();
    assert!(p.doc["scene"]["ext"]["editor"]["view3d"]["target"][0].as_f64().unwrap() < 0.0);
    edit(&mut p, json!({ "view": { "frame": "doll" } })).unwrap();
    assert_eq!(p.doc["scene"]["ext"]["editor"]["view3d"]["target"][0], json!(3.0));
    assert_eq!(p.doc["scene"]["ext"]["editor"]["view3d"]["target"][2], json!(0.0));
    assert!(edit(&mut p, json!({ "view": { "frame": "nothing" } })).is_err());
    assert!(edit(&mut p, json!({ "view": { "zoom": 0 } })).is_err());
    let mut scene = p.doc["scene"].clone();
    wprs_editor::stage3d::preview(&mut scene);
    assert_eq!(scene["camera3d"]["center"][0], json!(3.0));
    edit(&mut p, json!({ "view": { "through": true } })).unwrap();
    let mut scene = p.doc["scene"].clone();
    wprs_editor::stage3d::preview(&mut scene);
    assert!(scene.get("camera3d").is_none());
    edit(&mut p, json!({ "view": { "reset": true } })).unwrap();
    assert!(p.doc["scene"]["ext"]["editor"].get("view3d").is_none());
}

#[test]
fn tone_maps_need_hdr_and_the_split_preview_is_kept_for_the_editor() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    assert!(edit(&mut p, json!({ "tones": { "tonemap": "aces" } })).is_err());
    let before = p.doc.clone();
    edit(&mut p, json!({ "tones": { "hdr": true, "tonemap": "agx", "split": 0.5 } })).unwrap();
    undone(&mut p, &before);
    let c = &p.doc["scene"]["canvas"];
    assert_eq!((c["hdr"].clone(), c["tonemap"].clone()), (json!(true), json!("agx")));
    assert_eq!(p.doc["scene"]["ext"]["editor"]["tonemap_split"], json!(0.5));
    assert!(edit(&mut p, json!({ "tones": { "tonemap": "filmic" } })).is_err());
    assert!(edit(&mut p, json!({ "tones": { "split": 2 } })).is_err());
    edit(&mut p, json!({ "tones": { "split": 0 } })).unwrap();
    assert!(p.doc["scene"]["ext"]["editor"].get("tonemap_split").is_none());
    edit(&mut p, json!({ "tones": { "hdr": false, "tonemap": "none" } })).unwrap();
    assert!(
        p.doc["scene"]["canvas"].get("hdr").is_none() && p.doc["scene"]["canvas"].get("tonemap").is_none()
    );
}

fn near(v: &Value, want: [f64; 2], by: f64) -> bool {
    (v[0].as_f64().unwrap() - want[0]).abs() <= by && (v[1].as_f64().unwrap() - want[1]).abs() <= by
}

#[test]
fn the_authoring_view_shows_handles_where_the_models_are_and_the_camera_frustum() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    act_card(&mut p);
    let shown = ask(&p, json!({ "stage3d": { "time": 0 } }));
    let toy = &shown["models"][0];
    assert_eq!(toy["id"], "doll");
    assert!(near(&toy["at"], [960.0, 540.0 - 108.0], 60.0), "{toy}");
    let x = &toy["arms"][0];
    assert!(x[0].as_f64().unwrap() > toy["at"][0].as_f64().unwrap() + 30.0, "{toy}");
    let b = &toy["box"];
    assert!(b[0].as_f64().unwrap() < 960.0 && b[2].as_f64().unwrap() > 960.0, "{b}");
    assert!(shown["frustum"].as_array().unwrap().is_empty());
    edit(&mut p, json!({ "view": { "orbit": [60, 30], "zoom": 0.5 } })).unwrap();
    let shown = ask(&p, json!({ "stage3d": { "time": 0 } }));
    assert_eq!(shown["frustum"].as_array().unwrap().len(), 12);
    assert!(
        shown["handles"]["eye"].is_array() && shown["handles"]["center"].is_array(),
        "{}",
        shown["handles"]
    );
    assert_eq!(shown["flat"][0]["depth_test"], true);
    let _ = layer(&p, "doll");
}

fn act_card(p: &mut wprs_editor::Project) {
    edit(p, json!({ "group": { "id": "world", "layers": ["doll"] } })).unwrap();
    super::act(p, json!({ "add_layer": { "kind": "solid", "id": "card", "into": "world" } })).unwrap();
    edit(p, json!({ "depth_test": { "layer": "card", "on": true } })).unwrap();
}
