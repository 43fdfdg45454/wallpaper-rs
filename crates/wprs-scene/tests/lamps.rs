use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

fn scene(layers: Json) -> Scene {
    let assets = json!({ "wall": { "path": "wall.png" }, "song": { "path": "song.ogg" } });
    let timelines = [json!({ "id": "go", "duration": 2, "loop": "repeat" })];
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "assets": assets, "timelines": timelines,
        "layers": layers });
    Scene::from_json(&doc.to_string()).unwrap()
}

fn issues(layers: Json) -> Vec<String> {
    scene(layers).validate(&BTreeMap::new()).iter().map(ToString::to_string).collect()
}

fn lamp(id: &str, extra: Json) -> Json {
    let mut l = json!({ "id": id, "type": "light", "transform": { "position": [50, 50] } });
    l.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    l
}

#[test]
fn lighting_maps_must_be_pictures() {
    for map in ["roughness_map", "emissive_map"] {
        let wall = json!({ "id": "w", "type": "image", "image": "wall", "lighting": { map: "song" } });
        let found = issues(json!([wall]));
        assert!(
            found.iter().any(|i| i.contains(&format!("/layers/0/lighting/{map} does not fit"))),
            "{found:?}"
        );
        let missing = json!({ "id": "w", "type": "image", "image": "wall", "lighting": { map: "nothing" } });
        assert!(!issues(json!([missing])).is_empty());
    }
}

#[test]
fn lit_particles_still_name_their_own_feature() {
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "particles": { "snow": { "capacity": 10, "emitters": [{ "shape": "edge" }] } },
        "layers": [{ "id": "p", "type": "particles", "system": "snow", "lighting": {} }] });
    let scene = Scene::from_json(&doc.to_string()).unwrap();
    assert!(scene.features().contains("particle_lighting"));
}

#[test]
fn lamps_follow_their_layer_parents_and_visibility() {
    let group = json!({ "id": "g", "type": "group", "transform": { "position": [100, 0] }, "layers": [
        lamp("inside", json!({ "light": "spot", "transform": { "position": [10, 20], "rotation": 90 } })),
        lamp("off", json!({ "visible": false }))
    ]});
    let runtime =
        Runtime::new(Arc::new(scene(json!([lamp("top", json!({ "radius": 80 })), group]))), Values::new());
    let frame = runtime.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3);
    let names: Vec<&str> = frame.lamps.iter().map(|l| l.layer.id.as_str()).collect();
    assert_eq!(names, ["top", "inside"]);
    let inside = frame.lamps[1];
    assert_eq!(inside.at(), [110.0, 20.0]);
    let [x, y, z] = inside.heading();
    assert!((x + 1.0).abs() < 1e-4 && y.abs() < 1e-4 && z.abs() < 1e-4, "{x} {y} {z}");
    assert_eq!(frame.lamps[0].power(0.0), 1.0);
    assert_eq!(frame.lamps[0].power(80.0), 0.0);
    assert!((frame.lamps[0].power(40.0) - 0.5625).abs() < 1e-5);
}

#[test]
fn tilted_lamps_point_into_the_canvas() {
    let runtime = Runtime::new(
        Arc::new(scene(json!([lamp(
            "sun",
            json!({ "light": "directional", "transform": { "tilt": [30, 0] } })
        )]))),
        Values::new(),
    );
    let frame = runtime.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3);
    let [x, y, z] = frame.lamps[0].heading();
    assert!(x.abs() < 1e-5 && (y - 30f32.to_radians().cos()).abs() < 1e-5 && (z + 0.5).abs() < 1e-5);
}

#[test]
fn a_scene_keeps_at_most_twelve_lamps() {
    let lamps: Vec<Json> = (0..14).map(|i| lamp(&format!("l{i}"), json!({}))).collect();
    let runtime = Runtime::new(Arc::new(scene(json!(lamps))), Values::new());
    assert_eq!(runtime.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3).lamps.len(), MAX_LIGHTS);
}

#[test]
fn moving_lamps_ask_for_frames_except_at_the_lowest_level() {
    let walk = json!({ "timeline": "go", "keys": [[0, 1], [2, 4]] });
    let runtime =
        Runtime::new(Arc::new(scene(json!([lamp("l", json!({ "intensity": walk }))]))), Values::new());
    let view = [0.0, 0.0, 200.0, 100.0];
    assert_eq!(runtime.frame(view, Quality::Q3).wake, Wake::Frame);
    assert_eq!(runtime.frame(view, Quality::Q0).wake, Wake::Idle);
    let still = Runtime::new(Arc::new(scene(json!([lamp("l", json!({}))]))), Values::new());
    assert_eq!(still.frame(view, Quality::Q3).wake, Wake::Idle);
}

#[test]
fn ambient_and_sky_come_from_the_canvas() {
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [20, 10], "ambient": "#102030" }, "layers": [] });
    let runtime = Runtime::new(Arc::new(Scene::from_json(&doc.to_string()).unwrap()), Values::new());
    let frame = runtime.frame([0.0, 0.0, 20.0, 10.0], Quality::Q3);
    assert!(frame.ambient.is_some() && frame.sky.is_none());
}
