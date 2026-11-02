use super::*;

fn shared() -> Manifest {
    manifest(Kind::Assets, "assets.json", json!({ "requires": { "features": ["assets"] } }))
}

fn document(parts: Value) -> String {
    json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 64] },
        "layers": [{ "id": "glow", "type": "solid", "color": "#ffcc00" }],
        "particles": { "snow": { "capacity": 10, "emitters": [{ "shape": { "point": [0, 0] } }] } }, "ext": { "parts": parts } })
    .to_string()
}

#[test]
fn parts_are_listed_with_their_titles_and_thumbnails() {
    let png = image(32, 32, ImageFormat::Png);
    let parts = json!([{ "kind": "layer", "id": "glow", "title": "Glow", "thumbnail": "parts/glow.png" },
        { "kind": "particles", "id": "snow" }]);
    let text = document(parts);
    let mut package = load(&shared(), &[("assets.json", text.as_bytes()), ("parts/glow.png", &png)]).unwrap();
    let listed = wprs_format::parts(&package.scene().unwrap()).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].thumbnail.as_deref(), Some("parts/glow.png"));
    let plain = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [8, 8] },
        "layers": [{ "id": "a", "type": "solid", "color": "#000000" }], "particles": { "b": { "capacity": 10, "emitters": [{ "shape": { "point": [0, 0] } }] } } })
    .to_string();
    let mut package = load(&shared(), &[("assets.json", plain.as_bytes())]).unwrap();
    let kinds: Vec<_> =
        wprs_format::parts(&package.scene().unwrap()).unwrap().into_iter().map(|p| p.kind).collect();
    assert_eq!(kinds, ["layer", "particles"]);
}

#[test]
fn bad_parts_are_refused() {
    let png = image(32, 32, ImageFormat::Png);
    let big = image(2048, 8, ImageFormat::Png);
    let long = "x".repeat(129);
    let many: Vec<Value> = (0..257).map(|_| json!({ "kind": "layer", "id": "glow" })).collect();
    for (parts, extra) in [
        (json!([{ "kind": "sound", "id": "glow" }]), None),
        (json!([{ "kind": "layer", "id": "gone" }]), None),
        (json!([{ "kind": "layer", "id": "glow" }, { "kind": "layer", "id": "glow" }]), None),
        (json!([{ "kind": "layer", "id": "glow", "title": long }]), None),
        (json!([{ "kind": "layer", "id": "glow", "title": "<b>" }]), None),
        (json!([{ "kind": "layer", "id": "glow", "colour": 1 }]), None),
        (json!(many), None),
        (json!([{ "kind": "layer", "id": "glow", "thumbnail": "big.png" }]), Some(("big.png", &big))),
        (json!([{ "kind": "layer", "id": "glow", "thumbnail": "p.webp" }]), Some(("p.webp", &png))),
    ] {
        let text = document(parts.clone());
        let mut files: Vec<(&str, &[u8])> = vec![("assets.json", text.as_bytes())];
        files.extend(extra.map(|(p, b)| (p, b.as_slice())));
        assert!(load(&shared(), &files).is_err(), "{parts}");
    }
    let text = document(json!([{ "kind": "layer", "id": "glow", "thumbnail": "gone.png" }]));
    assert!(matches!(load(&shared(), &[("assets.json", text.as_bytes())]), Err(Error::Missing(_))));
    let text = document(json!([{ "kind": "layer", "id": "glow", "thumbnail": "../up.png" }]));
    assert!(load(&shared(), &[("assets.json", text.as_bytes())]).is_err());
}

#[test]
fn assets_packages_need_no_other_package() {
    let needs = json!({ "requires": { "features": ["assets", "packages"],
        "packages": [{ "id": "org.example.fx", "version": "^1" }] } });
    assert_eq!(field(manifest(Kind::Assets, "assets.json", needs).check()), "requires.packages");
}
