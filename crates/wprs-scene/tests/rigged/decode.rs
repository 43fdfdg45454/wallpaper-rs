use super::kit::{Gl, arm, fox, scene};
use serde_json::json;
use wprs_scene::{MAX_RIG_PALETTE, MAX_RIG_VERTICES, Rigged};

fn refused(gl: &Gl) -> String {
    Rigged::decode(&gl.bytes()).unwrap_err()
}

fn attribute(gl: &mut Gl, name: &str, accessor: usize) {
    gl.doc["meshes"][0]["primitives"][0]["attributes"][name] = json!(accessor);
}

#[test]
fn the_arm_decodes_into_bones_parts_shapes_and_clips() {
    let rig = Rigged::decode(&arm().bytes()).unwrap();
    assert_eq!((rig.joints.len(), rig.palette.len(), rig.pieces.len(), rig.vertex_count()), (5, 4, 2, 14));
    assert_eq!(
        (rig.parts.clone(), rig.shapes.clone()),
        (vec!["body".to_string(), "patch".into()], vec!["lift".to_string()])
    );
    assert_eq!((rig.motions[0].name.as_str(), rig.motions[0].length, rig.morphs.len()), ("wave", 1.0, 10));
    assert!(rig.descends(3, 1) && !rig.descends(1, 3));
    let scene = scene(fox(json!({})), arm().bytes()).unwrap();
    assert!(scene.rigs.contains_key("arm"));
}

#[test]
fn bone_cycles_and_nodes_with_two_parents_are_refused() {
    let mut gl = arm();
    gl.doc["nodes"][3]["children"] = json!([1]);
    assert!(refused(&gl).contains("cycle"), "{}", refused(&gl));
    let mut gl = arm();
    gl.doc["nodes"][4]["children"] = json!([3]);
    assert!(refused(&gl).contains("two parents"));
    let mut gl = arm();
    gl.doc["nodes"][2]["children"] = json!([2]);
    assert!(refused(&gl).contains("cycle"));
}

#[test]
fn weights_must_add_up_to_one_and_bones_and_indices_must_exist() {
    let mut gl = arm();
    let w = gl.floats("VEC4", &[[0.5f32, 0.2, 0.0, 0.0]; 10].concat());
    attribute(&mut gl, "WEIGHTS_0", w);
    assert!(refused(&gl).contains("add up to 0.7"), "{}", refused(&gl));
    let mut gl = arm();
    let w = gl.floats("VEC4", &[[1.0f32, 0.0, 0.0, 0.0]; 10].concat());
    let j = gl.joints(&[[7u16, 0, 0, 0]; 10].concat());
    attribute(&mut gl, "WEIGHTS_0", w);
    attribute(&mut gl, "JOINTS_0", j);
    assert!(refused(&gl).contains("missing bone 7"));
    let mut gl = arm();
    let i = gl.indices(&[0, 1, 99]);
    gl.doc["meshes"][0]["primitives"][0]["indices"] = json!(i);
    assert!(refused(&gl).contains("index out of the range"));
    let mut gl = arm();
    gl.doc["skins"][0]["joints"] = json!([1, 2, 30]);
    assert!(refused(&gl).contains("missing node 30"));
}

#[test]
fn values_that_are_not_numbers_and_unread_layouts_are_refused() {
    let mut gl = arm();
    let p = gl.floats("VEC3", &[[f32::NAN, 0.0, 0.0]; 10].concat());
    attribute(&mut gl, "POSITION", p);
    assert!(refused(&gl).contains("not a number"));
    let mut gl = arm();
    gl.doc["accessors"][0]["sparse"] = json!({ "count": 1 });
    assert!(refused(&gl).contains("sparse"));
    let mut gl = arm();
    gl.doc["meshes"][1]["primitives"][0]["mode"] = json!(1);
    assert!(refused(&gl).contains("other than triangles"));
    let mut gl = arm();
    gl.doc["accessors"][0]["count"] = json!(1000);
    assert!(refused(&gl).contains("past its buffer view"));
    let mut gl = arm();
    gl.doc["nodes"][1]["translation"] = json!([1e39, 0.0, 0.0]);
    assert!(Rigged::decode(&gl.bytes()).is_err());
    let mut gl = arm();
    let backwards = gl.floats("SCALAR", &[1.0, 0.0]);
    gl.doc["animations"][0]["samplers"][0]["input"] = json!(backwards);
    assert!(refused(&gl).contains("unsorted"));
}

#[test]
fn meshes_are_capped_in_vertices_bones_and_parts() {
    let mut gl = Gl::new();
    let many = gl.floats("VEC3", &vec![0.0; (MAX_RIG_VERTICES + 1) * 3]);
    gl.doc["nodes"] = json!([{ "mesh": 0 }]);
    gl.doc["meshes"] = json!([{ "primitives": [{ "attributes": { "POSITION": many } }] }]);
    assert!(refused(&gl).contains("vertices"), "{}", refused(&gl));
    let mut gl = Gl::new();
    let one = gl.floats("VEC3", &[0.0; 9]);
    let nodes: Vec<_> = (0..=MAX_RIG_PALETTE).map(|_| json!({ "mesh": 0 })).collect();
    gl.doc["nodes"] = json!(nodes);
    gl.doc["meshes"] = json!([{ "primitives": [{ "attributes": { "POSITION": one } }] }]);
    assert!(refused(&gl).contains("bones and parts"), "{}", refused(&gl));
}
