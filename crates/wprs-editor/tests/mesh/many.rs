use super::kit::{act, info, project};
use serde_json::json;

fn quads(count: usize) -> Vec<u8> {
    let mut bin = Vec::new();
    let quad = |i: usize| {
        let x = i as f32 * 4.0 - 128.0;
        [x, -20.0, 0.0, x + 3.0, -20.0, 0.0, x + 3.0, 20.0, 0.0, x, 20.0, 0.0]
    };
    (0..count).for_each(|i| bin.extend(quad(i).iter().flat_map(|v| v.to_le_bytes())));
    let at = bin.len();
    bin.extend([0u32, 1, 2, 0, 2, 3].iter().flat_map(|i| i.to_le_bytes()));
    let views: Vec<_> =
        (0..count).map(|i| json!({ "buffer": 0, "byteOffset": i * 48, "byteLength": 48 })).collect();
    let mut views = views;
    views.push(json!({ "buffer": 0, "byteOffset": at, "byteLength": 24 }));
    let mut accessors: Vec<_> = (0..count)
        .map(|i| json!({ "bufferView": i, "type": "VEC3", "componentType": 5126, "count": 4 }))
        .collect();
    accessors.push(json!({ "bufferView": count, "type": "SCALAR", "componentType": 5125, "count": 6 }));
    let meshes: Vec<_> = (0..count)
        .map(|i| json!({ "primitives": [{ "attributes": { "POSITION": i }, "indices": count }] }))
        .collect();
    let nodes: Vec<_> = (0..count).map(|i| json!({ "name": format!("part{i}"), "mesh": i })).collect();
    let doc = json!({ "asset": { "version": "2.0" }, "nodes": nodes, "meshes": meshes, "accessors": accessors,
        "bufferViews": views, "buffers": [{ "byteLength": bin.len() }] });
    let mut text = doc.to_string().into_bytes();
    text.resize(text.len().div_ceil(4) * 4, b' ');
    let mut out = Vec::new();
    for w in [0x4654_6C67u32, 2, (28 + text.len() + bin.len()) as u32, text.len() as u32, 0x4E4F_534A] {
        out.extend(w.to_le_bytes());
    }
    out.extend(text);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(0x004E_4942u32.to_le_bytes());
    out.extend(bin);
    out
}

#[test]
fn every_mesh_the_scene_draws_is_edited_and_models_keep_the_bones_of_their_file() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    std::fs::write(p.dir.join("many.glb"), quads(wprs_scene::MAX_RIG_PIECES)).unwrap();
    act(&mut p, json!({ "set": { "pointer": "/scene/assets/many", "value": { "path": "many.glb" } } }))
        .unwrap();
    let layer = json!({ "id": "crowd", "type": "mesh", "mesh": "many", "size": [256, 64] });
    act(&mut p, json!({ "add": { "pointer": "/scene/layers/-", "value": layer } })).unwrap();
    assert_eq!(
        info(&p, "crowd")["editable"],
        json!(true),
        "a mesh of {} primitives",
        wprs_scene::MAX_RIG_PIECES
    );
    act(&mut p, json!({ "bone": { "layer": "crowd", "name": "spine", "at": [0.5, 0.5] } })).unwrap();
    act(&mut p, json!({ "auto_weights": { "layer": "crowd" } })).unwrap();
    assert!(info(&p, "crowd")["bones"].as_array().unwrap().iter().any(|b| b["name"] == "spine"));
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/toy.glb");
    std::fs::copy(fixture, p.dir.join("toy.glb")).unwrap();
    act(&mut p, json!({ "set": { "pointer": "/scene/assets/toy", "value": { "path": "toy.glb" } } }))
        .unwrap();
    let model = json!({ "id": "toy", "type": "model", "model": "toy" });
    act(&mut p, json!({ "add": { "pointer": "/scene/layers/-", "value": model } })).unwrap();
    let shown = info(&p, "toy");
    assert_eq!((&shown["model"], &shown["editable"]), (&json!(true), &json!(false)), "{shown}");
    let refused = act(&mut p, json!({ "bone": { "layer": "toy", "name": "extra", "at": [0.5, 0.5] } }));
    assert!(refused.is_err(), "the bones of a model come from its file");
}
