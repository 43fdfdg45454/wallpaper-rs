use super::kit::{Gl, arm, fox, scene};
use serde_json::json;
use std::sync::Arc;
use wprs_scene::{MAX_RIG_INDICES, MAX_RIG_NODES, MAX_RIG_PIECES, Rigged, Runtime, Values};

fn refused(gl: &Gl) -> String {
    Rigged::decode(&gl.bytes()).unwrap_err()
}

fn shaped(names: &[&str]) -> Gl {
    let mut gl = arm();
    let lift = gl.doc["meshes"][0]["primitives"][0]["targets"][0].clone();
    gl.doc["meshes"][0]["primitives"][0]["targets"] = json!(vec![lift; names.len()]);
    gl.doc["meshes"][0]["extras"]["targetNames"] = json!(names);
    let keys: Vec<f32> = (0..2 * names.len()).map(|k| (k / names.len()) as f32).collect();
    let lifts = gl.floats("SCALAR", &keys);
    gl.doc["animations"][0]["samplers"][1]["output"] = json!(lifts);
    gl
}

#[test]
fn a_mesh_may_not_name_a_blend_shape_twice() {
    let found = refused(&shaped(&["a", "a", "a", "a", "a", "a"]));
    assert!(found.contains("\"a\" twice"), "{found}");
}

#[test]
fn a_weights_track_wider_than_four_shapes_plays() {
    let gl = shaped(&["a", "b", "c", "d", "e", "f"]);
    let layer =
        fox(json!({ "clips": [{ "id": "b", "animation": "wave" }], "animations": [{ "clip": "b" }] }));
    let mut rt = Runtime::new(Arc::new(scene(layer, gl.bytes()).unwrap()), Values::new());
    rt.advance(0.5);
    let morphs = &rt.puppet("fox").unwrap().posed().morphs[0];
    assert!(morphs.iter().filter(|m| (m.1 - 0.5).abs() < 1e-4).count() >= 6, "{morphs:?}");
}

#[test]
fn triangle_corners_are_capped_before_they_are_read() {
    let mut gl = Gl::new();
    let one = gl.floats("VEC3", &[0.0; 9]);
    let many = gl.indices(&vec![0; MAX_RIG_INDICES / 2 + 3]);
    gl.doc["nodes"] = json!([{ "mesh": 0 }, { "mesh": 0 }]);
    gl.doc["meshes"] = json!([{ "primitives": [{ "attributes": { "POSITION": one }, "indices": many }] }]);
    assert!(refused(&gl).contains("triangle corners"), "{}", refused(&gl));
    gl.doc["nodes"] = json!([{ "mesh": 0 }]);
    assert!(Rigged::decode(&gl.bytes()).is_ok());
    gl.doc["meshes"][0]["primitives"][0]["mode"] = json!(5);
    gl.doc["nodes"] = json!([{ "mesh": 0 }]);
    assert!(refused(&gl).contains("triangle corners"), "strips make three corners an index");
}

#[test]
fn drawn_primitives_and_nodes_are_capped() {
    let mut gl = Gl::new();
    let one = gl.floats("VEC3", &[0.0; 9]);
    let nodes: Vec<_> = (0..=MAX_RIG_PIECES).map(|_| json!({ "mesh": 0 })).collect();
    gl.doc["nodes"] = json!(nodes);
    gl.doc["meshes"] = json!([{ "primitives": [{ "attributes": { "POSITION": one } }] }]);
    assert!(refused(&gl).contains("drawn primitives"), "{}", refused(&gl));
    let mut gl = Gl::new();
    gl.doc["nodes"] = json!(vec![json!({}); MAX_RIG_NODES + 1]);
    assert!(refused(&gl).contains("nodes"), "{}", refused(&gl));
    gl.doc["nodes"] = json!(vec![json!({}); MAX_RIG_NODES]);
    assert!(Rigged::decode(&gl.bytes()).is_ok());
}
