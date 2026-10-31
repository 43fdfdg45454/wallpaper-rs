#[path = "../common/mod.rs"]
mod common;

use common::{archive, image, v2};
use image::ImageFormat;
use serde_json::{Value, json};
use std::io::Cursor;
use wprs_format::{Error, Kind, Limits, Manifest, Package, Platform, features, read};

mod pages;
mod parts;

fn manifest(kind: Kind, entry: &str, more: Value) -> Manifest {
    let mut value = serde_json::to_value(Manifest { entry: entry.into(), ..v2(kind) }).unwrap();
    value.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

fn load(manifest: &Manifest, files: &[(&str, &[u8])]) -> Result<Package<Cursor<Vec<u8>>>, Error> {
    let json = manifest.to_json().unwrap();
    let png = image(16, 16, ImageFormat::Png);
    let base: [(&str, &[u8]); 2] = [("manifest.json", json.as_bytes()), ("preview.png", &png)];
    read(Cursor::new(archive(&[&base[..], files].concat())), &Limits::default())
}

fn field<T: std::fmt::Debug>(result: Result<T, Error>) -> String {
    match result {
        Err(Error::Invalid { field, .. }) => field,
        other => panic!("accepted: {other:?}"),
    }
}

fn page(more: Value) -> Manifest {
    let mut base = json!({ "inputs": ["audio", "network"], "requires": { "features": ["inputs.audio", "inputs.network", "web"] },
        "web": { "preload": "compat.js", "background": "#000000", "storage_kib": 512 },
        "permissions": { "network": { "allow": ["https://api.example.org", "https://tiles.example.org:8443"] } } });
    base.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    manifest(Kind::Web, "index.html", base)
}

fn preset(requires: Value) -> Manifest {
    let base = json!({ "features": ["packages", "preset"], "packages": [{ "id": "org.example.dusk", "version": "^1.2" }] });
    manifest(
        Kind::Preset,
        "preset.json",
        json!({ "requires": if requires.is_null() { base } else { requires } }),
    )
}
