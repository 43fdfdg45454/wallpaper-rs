#[path = "../common/mod.rs"]
mod common;

use common::{archive, image, manifest as v1, v2};
use image::ImageFormat;
use serde_json::{Value, json};
use std::io::Cursor;
use wprs_format::{Error, Kind, Limits, Manifest, Package, Platform, features, read};

mod hostile;
mod reading;

const RIPPLE: &str = "@fragment\nfn fs_ripple(in: VertexOut) -> @location(0) vec4<f32> {\n    \
    return tap(in.uv) * (0.5 + 0.5 * sin(frame.time));\n}\n";

fn scene(layers: Value, more: Value) -> String {
    let mut scene = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] },
        "assets": { "sky": { "path": "sky.png" } }, "layers": layers });
    scene.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    scene.to_string()
}

fn manifest(features: &[&str]) -> Manifest {
    let requires = json!({ "features": features });
    let mut value = serde_json::to_value(v2(Kind::Scene)).unwrap();
    value["requires"] = requires;
    serde_json::from_value(value).unwrap()
}

fn load(manifest: &Manifest, files: &[(&str, &[u8])]) -> Result<Package<Cursor<Vec<u8>>>, Error> {
    let json = manifest.to_json().unwrap();
    let png = image(16, 16, ImageFormat::Png);
    let base: [(&str, &[u8]); 3] =
        [("manifest.json", json.as_bytes()), ("preview.png", &png), ("sky.png", &png)];
    read(Cursor::new(archive(&[&base[..], files].concat())), &Limits::default())
}
