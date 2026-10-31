#[path = "../common/mod.rs"]
mod common;

use common::{archive, image, v2};
use image::ImageFormat;
use serde_json::{Value, json};
use std::io::Cursor;
use wprs_format::{Error, Input, Kind, Limits, Manifest, Package, Platform, features, read};

mod inputs;
mod released;

fn glb(doc: &Value) -> Vec<u8> {
    let mut text = doc.to_string().into_bytes();
    text.resize(text.len().div_ceil(4) * 4, b' ');
    let mut out = b"glTF".to_vec();
    for word in [2, 20 + text.len() as u32, text.len() as u32, 0x4E4F_534A] {
        out.extend(word.to_le_bytes());
    }
    out.extend(text);
    out
}

fn fox() -> Vec<u8> {
    glb(&json!({ "asset": { "version": "2.0" }, "nodes": [{ "name": "arm" }, { "name": "hand" }],
        "skins": [{ "joints": [0, 1] }], "accessors": [{ "count": 3 }],
        "meshes": [{ "name": "body", "primitives": [{ "attributes": { "POSITION": 0, "JOINTS_0": 0, "WEIGHTS_0": 0 } }] }] }))
}

fn manifest(features: &[&str], change: Value) -> Manifest {
    let mut value = serde_json::to_value(v2(Kind::Scene)).unwrap();
    value["requires"] = json!({ "features": features });
    value.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

fn load(
    manifest: &Manifest,
    scene: &Value,
    files: &[(&str, &[u8])],
) -> Result<Package<Cursor<Vec<u8>>>, Error> {
    load_raw(&serde_json::to_value(manifest).unwrap(), scene, files)
}

fn load_raw(
    manifest: &Value,
    scene: &Value,
    files: &[(&str, &[u8])],
) -> Result<Package<Cursor<Vec<u8>>>, Error> {
    let (json, text, png) = (manifest.to_string(), scene.to_string(), image(16, 16, ImageFormat::Png));
    let base: [(&str, &[u8]); 4] = [
        ("manifest.json", json.as_bytes()),
        ("preview.png", &png),
        ("sky.png", &png),
        ("scene.json", text.as_bytes()),
    ];
    read(Cursor::new(archive(&[&base[..], files].concat())), &Limits::default())
}

fn scene(layers: Value, more: Value) -> Value {
    let mut scene = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] },
        "assets": { "sky": { "path": "sky.png" } }, "layers": layers });
    scene.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    scene
}
