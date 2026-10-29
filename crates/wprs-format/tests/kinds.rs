mod common;

use common::{archive, image, v2};
use image::ImageFormat;
use serde_json::{Value, json};
use std::io::Cursor;
use wprs_format::{Error, Kind, Limits, Manifest, Package, Platform, features, read};

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

#[test]
fn web_packages_declare_what_their_page_uses() {
    let html: &[(&str, &[u8])] =
        &[("index.html", b"<!doctype html>"), ("compat.js", b"x"), ("app.wasm", b"\0asm\x01")];
    let package = load(&page(json!({})), html).unwrap();
    assert_eq!(package.facts.features.iter().collect::<Vec<_>>(), ["inputs.audio", "inputs.network", "web"]);
    assert!(
        package.facts.facets.contains(&"audio".into()) && package.facts.facets.contains(&"network".into())
    );
    assert!(features::warnings(&package.manifest, &package.facts).is_empty());
    let refused = features::playable(&package.manifest, &package.facts, Platform::Desktop).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "this package needs a newer version of wallpaper-rs: it uses the feature web (is a web page)"
    );
    assert!(!package.facts.facets.contains(&"desktop".into()));
    let bad = [("app.wasm", b"MZ\x90\0".as_slice()), ("index.html", b"x"), ("compat.js", b"x")];
    assert_eq!(field(load(&page(json!({})), &bad)), "app.wasm");
    let checked = |more: Value| field(page(more).check());
    assert_eq!(checked(json!({ "web": { "storage_kib": 5121 } })), "web.storage_kib");
    assert_eq!(checked(json!({ "web": { "background": "black" } })), "web.background");
    let origins = |allow: Value| checked(json!({ "permissions": { "network": { "allow": allow } } }));
    for allow in
        [json!(["http://example.org"]), json!(["https://example.org/api"]), json!(["https://Example.org"])]
    {
        assert_eq!(origins(allow), "permissions.network.allow");
    }
    let many: Vec<String> = (0..9).map(|i| format!("https://a{i}.example.org")).collect();
    assert_eq!(origins(json!(many)), "permissions.network.allow");
    assert_eq!(checked(json!({ "inputs": ["audio"] })), "inputs");
    let scene = |more: Value| field(manifest(Kind::Scene, "scene.json", more).check());
    assert_eq!(scene(json!({ "web": {} })), "web");
    let networked = json!({ "permissions": { "network": { "allow": ["https://api.example.org"] } } });
    assert_eq!(scene(networked), "permissions.network");
    assert_eq!(scene(json!({ "display": { "span": "per-output" } })), "display.span");
    let spread = manifest(Kind::Scene, "scene.json", json!({ "display": { "span": "all-outputs" } }));
    let named = wprs_format::features::facts(&spread, None, None).features;
    assert!(named.contains("spanned_scenes") && !named.contains("display2"), "{named:?}");
    let picture = field(manifest(Kind::Image, "a.png", json!({ "permissions": { "rgb": true } })).check());
    assert_eq!(picture, "permissions");
    let script = [("index.html", b"x".as_slice()), ("compat.js", b"x")];
    let preload = page(json!({ "web": { "preload": "index.html" } }));
    assert_eq!(field(load(&preload, &script).map(drop)), "web.preload");
    let executable = [("index.html", b"x".as_slice()), ("compat.js", b"x"), ("tool.exe", b"MZ")];
    assert!(matches!(load(&page(json!({})), &executable), Err(Error::Extension { .. })));
}

fn preset(requires: Value) -> Manifest {
    let base = json!({ "features": ["packages", "preset"], "packages": [{ "id": "org.example.dusk", "version": "^1.2" }] });
    manifest(
        Kind::Preset,
        "preset.json",
        json!({ "requires": if requires.is_null() { base } else { requires } }),
    )
}

#[test]
fn presets_carry_values_and_pictures_for_a_base_package() {
    let png = image(8, 8, ImageFormat::Png);
    let file = json!({ "base": { "id": "org.example.dusk", "version": "^1.2" }, "values": { "tint": "#3d5aff", "speed": 0.4 },
        "files": { "photo": "photo.png" } }).to_string();
    let files: [(&str, &[u8]); 2] = [("preset.json", file.as_bytes()), ("photo.png", &png)];
    let mut package = load(&preset(Value::Null), &files).unwrap();
    assert_eq!(package.preset().unwrap().base.id, "org.example.dusk");
    assert_eq!(package.facts.features.iter().collect::<Vec<_>>(), ["packages", "preset"]);
    assert!(features::playable(&package.manifest, &package.facts, Platform::Android).is_err());
    let other = json!({ "features": ["packages", "preset"], "packages": [{ "id": "org.example.dawn", "version": "1" }] });
    assert_eq!(field(load(&preset(other), &files).map(drop)), "requires.packages");
    assert_eq!(field(preset(json!({ "features": ["preset"] })).check()), "requires.packages");
    let gone = file.replace("photo.png", "gone.png");
    assert!(matches!(
        load(&preset(Value::Null), &[("preset.json", gone.as_bytes())]),
        Err(Error::Missing(_))
    ));
    let text = file.replace("photo.png", "notes.txt");
    let notes: [(&str, &[u8]); 2] = [("preset.json", text.as_bytes()), ("notes.txt", b"x")];
    assert_eq!(field(load(&preset(Value::Null), &notes).map(drop)), "preset.files.photo");
    let twice = file.replace("\"photo\":", "\"speed\":");
    let twice: [(&str, &[u8]); 2] = [("preset.json", twice.as_bytes()), ("photo.png", &png)];
    assert_eq!(field(load(&preset(Value::Null), &twice).map(drop)), "preset.files.speed");
    let props = Manifest {
        properties: serde_json::from_value(json!({ "on": { "type": "bool", "default": true } })).unwrap(),
        ..preset(Value::Null)
    };
    assert_eq!(field(props.check()), "properties");
    let script = [("preset.json", file.as_bytes()), ("photo.png", &png), ("run.js", b"x")];
    assert!(matches!(load(&preset(Value::Null), &script), Err(Error::Extension { .. })));
}

#[test]
fn packages_name_the_packages_they_need() {
    let needs = |packages: Value| {
        let requires = json!({ "features": ["packages"], "packages": packages });
        manifest(Kind::Scene, "scene.json", json!({ "requires": requires })).check()
    };
    needs(json!([{ "id": "org.example.fx", "version": ">=1.0, <2" }])).unwrap();
    for bad in [
        json!([{ "id": "Org.Example", "version": "1" }]),
        json!([{ "id": "org.example.sunset", "version": "1" }]),
        json!([{ "id": "org.example.fx", "version": "one" }]),
        json!([{ "id": "org.example.fx", "version": "1" }, { "id": "org.example.fx", "version": "2" }]),
        json!(
            (0..17).map(|i| json!({ "id": format!("org.example.p{i}"), "version": "1" })).collect::<Vec<_>>()
        ),
    ] {
        assert_eq!(field(needs(bad.clone())), "requires.packages", "{bad}");
    }
    let unknown = json!({ "features": ["packages"], "packages": [{ "id": "org.example.fx", "version": "1", "hash": 1 }] });
    let json = manifest(Kind::Scene, "scene.json", json!({})).to_json().unwrap();
    let mut value: Value = serde_json::from_str(&json).unwrap();
    value["requires"] = unknown;
    assert!(matches!(Manifest::from_json(value.to_string().as_bytes()), Err(Error::Json(_))));
    value["requires"]["features"] = json!(["packages", "holograms"]);
    assert!(matches!(Manifest::from_json(value.to_string().as_bytes()), Err(Error::Newer(_))));
}

#[test]
fn platforms_only_narrow_where_a_package_plays() {
    let phone = |platforms: Value| {
        manifest(
            Kind::Image,
            "image.png",
            json!({ "platforms": platforms, "requires": { "features": ["platforms"] } }),
        )
    };
    let png = image(64, 32, ImageFormat::Png);
    let package = load(&phone(json!(["android"])), &[("image.png", &png)]).unwrap();
    assert_eq!(package.facts.facets, ["android", "landscape"]);
    let refused = features::playable(&package.manifest, &package.facts, Platform::Desktop).unwrap_err();
    assert_eq!(refused.to_string(), "this package is made for Android only");
    features::playable(&package.manifest, &package.facts, Platform::Android).unwrap();
    assert_eq!(field(phone(json!(["android", "android"])).check()), "platforms");
    assert!(serde_json::from_value::<Manifest>(serde_json::to_value(phone(json!([]))).unwrap()).is_ok());
}

#[test]
fn assets_packages_offer_scene_parts() {
    let scene = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 64] },
        "layers": [{ "id": "glow", "type": "solid", "color": "#ffcc00" }] })
    .to_string();
    let shared = manifest(Kind::Assets, "assets.json", json!({ "requires": { "features": ["assets"] } }));
    let mut package = load(&shared, &[("assets.json", scene.as_bytes())]).unwrap();
    assert_eq!(package.scene().unwrap().layers.len(), 1);
    assert!(package.facts.facets.iter().all(|f| !["android", "desktop", "animated"].contains(&f.as_str())));
    let shown = Manifest {
        display: serde_json::from_value(json!({ "accent": "#ff0000" })).unwrap(),
        ..shared.clone()
    };
    assert_eq!(field(shown.check()), "display");
}

#[test]
fn compressed_textures_are_refused_until_they_are_drawn() {
    let entry = manifest(Kind::Image, "image.ktx2", json!({ "requires": { "features": ["ktx2"] } }));
    let package = load(&entry, &[("image.ktx2", b"\xabKTX 20\xbb")]).unwrap();
    let refused = features::playable(&package.manifest, &package.facts, Platform::Desktop).unwrap_err();
    assert!(refused.to_string().ends_with("the feature ktx2 (uses compressed textures in the KTX2 format)"));
    let unlisted = manifest(Kind::Image, "image.avif", json!({}));
    assert_eq!(field(load(&unlisted, &[("image.avif", b"x")]).map(drop)), "requires.features");
}

#[test]
fn folders_get_the_checks_of_packages() {
    let check = |manifest: &Manifest, files: &[(&str, &[u8])]| {
        let dir = tempfile::tempdir().unwrap();
        let (png, json) = (image(16, 16, ImageFormat::Png), manifest.to_json().unwrap());
        let base: [(&str, &[u8]); 2] = [("manifest.json", json.as_bytes()), ("preview.png", &png)];
        for (path, bytes) in base.iter().chain(files) {
            std::fs::write(dir.path().join(path), bytes).unwrap();
        }
        wprs_format::inspect(dir.path()).map(drop)
    };
    let html: [(&str, &[u8]); 3] = [("index.html", b"x"), ("compat.js", b"x"), ("app.wasm", b"notwasm")];
    assert_eq!(field(check(&page(json!({})), &html)), "app.wasm");
    let file =
        json!({ "base": { "id": "org.example.dusk", "version": "^1.2" }, "files": { "photo": "gone.png" } });
    let text = file.to_string();
    assert!(matches!(
        check(&preset(Value::Null), &[("preset.json", text.as_bytes())]),
        Err(Error::Missing(_))
    ));
    let notes = text.replace("gone.png", "notes.txt");
    let files: [(&str, &[u8]); 2] = [("preset.json", notes.as_bytes()), ("notes.txt", b"x")];
    assert_eq!(field(check(&preset(Value::Null), &files)), "preset.files.photo");
}
