use super::{act, at, project, refused};
use serde_json::json;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures")).join(name)
}

#[test]
fn a_mesh_layer_comes_from_a_glb_file_in_one_step_and_plays_a_new_clip() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let file = fixture("parts.glb");
    act(&mut p, json!({"add_layer": {"kind": "mesh", "id": "body", "file": file}})).unwrap();
    let path = wprs_editor::layers::find(&p.doc, "body").unwrap();
    let asset = at(&p, &format!("{path}/mesh"));
    assert_eq!(at(&p, &format!("/scene/assets/{}/path", asset.as_str().unwrap())), "assets/parts.glb");
    assert!(p.dir.join("assets/parts.glb").is_file());
    assert_eq!(p.history.labels().0.len(), 2, "importing and adding is one step");
    act(&mut p, json!({"clip": {"layer": "body", "id": "wave", "add": true, "play": true}})).unwrap();
    assert_eq!(at(&p, &format!("{path}/animations")), json!([{"clip": "wave"}]));
    refused(&mut p, json!({"clip": {"layer": "body", "id": "wave", "play": true}}));
    let picture = root.path().join("dot.png");
    image::RgbaImage::new(4, 4).save(&picture).unwrap();
    let wrong = refused(&mut p, json!({"add_layer": {"kind": "mesh", "file": picture}}));
    assert!(wrong.contains("cannot show"), "{wrong}");
    act(&mut p, json!({"add_layer": {"kind": "image", "id": "dot", "file": picture}})).unwrap();
    assert_eq!(at(&p, &format!("{}/image", wprs_editor::layers::find(&p.doc, "dot").unwrap())), "dot_file");
    let broken = root.path().join("broken.glb");
    std::fs::write(&broken, b"glTF nothing").unwrap();
    refused(&mut p, json!({"add_layer": {"kind": "mesh", "file": broken}}));
    refused(&mut p, json!({"add_layer": {"kind": "text", "file": picture}}));
}

#[test]
fn a_script_that_moves_the_camera_or_the_parallax_declares_camera() {
    let uses = |source: &str| wprs_editor::scripts::declaration(source)["uses"].clone();
    let parallax =
        "import { scene } from \"wprs\";\nexport function update() { scene.parallax = { amount: 2 }; }";
    assert!(uses(parallax).as_array().unwrap().contains(&json!("camera")), "{}", uses(parallax));
    assert!(
        uses("export function update() { wprs.scene.bloom.strength = 1; }").to_string().contains("camera")
    );
    assert!(!uses("export function update() { const sceneryX = 1; }").to_string().contains("camera"));
    assert!(!uses("export function update() { scene.cameraman = 1; }").to_string().contains("camera"));
}
