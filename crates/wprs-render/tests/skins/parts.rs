use super::kit::{mesh, near, render, renderer, rgb, scene};
use serde_json::json;
use wprs_scene::Quality;

fn quads() -> Vec<u8> {
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut add = |values: Vec<u8>| {
        views.push(json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": values.len() }));
        bin.extend(values);
        views.len() - 1
    };
    let quad = |x0: f32, x1: f32| [x0, -24.0, 0.0, x1, -24.0, 0.0, x1, 24.0, 0.0, x0, 24.0, 0.0];
    let f = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let (back, front) = (add(f(&quad(-24.0, 8.0))), add(f(&quad(-8.0, 24.0))));
    let list = add([0u32, 1, 2, 0, 2, 3].iter().flat_map(|i| i.to_le_bytes()).collect());
    let accessor = |view: usize, kind: &str, component: u32, count: usize| json!({ "bufferView": view, "type": kind, "componentType": component, "count": count });
    let primitive =
        |p: usize, m: usize| json!({ "attributes": { "POSITION": p }, "indices": 2, "material": m });
    let doc = json!({ "asset": { "version": "2.0" },
        "nodes": [{ "name": "back", "mesh": 0 }, { "name": "front", "mesh": 1 }],
        "meshes": [{ "name": "back", "primitives": [primitive(0, 0)] }, { "name": "front", "primitives": [primitive(1, 1)] }],
        "materials": [{ "pbrMetallicRoughness": { "baseColorFactor": [1, 0, 0, 1] } },
                      { "pbrMetallicRoughness": { "baseColorFactor": [0, 1, 0, 1] } }],
        "accessors": [accessor(back, "VEC3", 5126, 4), accessor(front, "VEC3", 5126, 4), accessor(list, "SCALAR", 5125, 6)],
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

fn drawn(parts: serde_json::Value) -> Vec<u8> {
    let mut layer = mesh(json!({ "parts": parts }));
    layer.as_object_mut().unwrap().remove("texture");
    render(renderer().unwrap(), scene(layer, quads()), 0.0, Quality::Q3)
}

#[test]
fn parts_draw_in_file_order_unless_their_order_says_otherwise() {
    if renderer().is_none() {
        return;
    }
    let (red, green) = ([255, 0, 0], [0, 255, 0]);
    let plain = drawn(json!({}));
    assert!(
        near(rgb(&plain, 12, 32), red)
            && near(rgb(&plain, 32, 32), green)
            && near(rgb(&plain, 50, 32), green)
    );
    let raised = drawn(json!({ "back": { "order": 1 } }));
    assert!(near(rgb(&raised, 32, 32), red) && near(rgb(&raised, 50, 32), green));
    let keyed = json!({ "back": { "order": { "keys": [[0, 2], [1, 2]] } } });
    assert!(near(rgb(&drawn(keyed), 32, 32), red));
}

#[test]
fn a_cutout_shows_its_part_only_over_the_part_it_follows() {
    if renderer().is_none() {
        return;
    }
    let cut = drawn(json!({ "front": { "clip_to": "back" } }));
    assert!(near(rgb(&cut, 12, 32), [255, 0, 0]) && near(rgb(&cut, 32, 32), [0, 255, 0]));
    assert!(near(rgb(&cut, 50, 32), [0, 0, 0]), "{:?}", rgb(&cut, 50, 32));
    let hidden = drawn(json!({ "back": { "visible": false } }));
    assert!(near(rgb(&hidden, 12, 32), [0, 0, 0]) && near(rgb(&hidden, 50, 32), [0, 255, 0]));
}
