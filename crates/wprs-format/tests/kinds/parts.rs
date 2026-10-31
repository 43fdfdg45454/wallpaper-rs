use super::*;

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
    let es = wprs_core::message::tr_in(&["es".into()]);
    assert!(refused.message().told(&es).starts_with("este paquete necesita una versión más reciente"));
    assert!(
        refused.message().told(&es).ends_with("la función ktx2 (usa texturas comprimidas en formato KTX2)")
    );
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
