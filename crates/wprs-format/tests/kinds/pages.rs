use super::*;

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
    for platform in [Platform::Desktop, Platform::Android] {
        features::playable(&package.manifest, &package.facts, platform).unwrap();
    }
    let facets = ["desktop", "android", "storage"];
    assert!(facets.iter().all(|f| package.facts.facets.contains(&(*f).into())), "{:?}", package.facts.facets);
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
