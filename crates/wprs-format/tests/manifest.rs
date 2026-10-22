mod common;

use common::manifest;
use wprs_format::{Control, Error, Kind, Manifest, Property, Result};

fn with(edit: impl FnOnce(&mut Manifest)) -> Result<()> {
    let mut manifest = manifest(Kind::Image);
    edit(&mut manifest);
    manifest.check()
}

fn field(result: Result<()>) -> String {
    match result {
        Err(Error::Invalid { field, .. }) => field,
        other => panic!("expected an invalid field, got {other:?}"),
    }
}

#[test]
fn valid_manifest_roundtrips_through_json() {
    let mut manifest = manifest(Kind::Scene);
    manifest
        .properties
        .insert("tint".into(), Property::new(Control::Color { default: "#ff8800".into(), alpha: false }));
    manifest.check().unwrap();
    assert_eq!(Manifest::from_json(manifest.to_json().unwrap().as_bytes()).unwrap(), manifest);
}

#[test]
fn parses_minimal_json_with_typed_properties() {
    let json =
        br#"{"schema":"wallpaper-rs.wpak/1","id":"org.example.waves","version":"0.2.0","title":"Waves",
        "license":"MIT","kind":"scene","entry":"scene.json","properties":{"speed":{"type":"slider",
        "min":0,"max":2,"step":0.25,"default":1},"dark":{"type":"bool","default":false}}}"#;
    let manifest = Manifest::from_json(json).unwrap();
    assert_eq!(manifest.kind, Kind::Scene);
    assert_eq!(
        manifest.properties["speed"],
        Property::new(Control::Slider {
            min: 0.0,
            max: 2.0,
            step: 0.25,
            default: 1.0,
            integer: false,
            unit: None
        })
    );
    assert_eq!(manifest.properties["dark"], Property::new(Control::Bool { default: false }));
    manifest.check().unwrap();
}

#[test]
fn unknown_fields_and_kinds_are_rejected() {
    let json = manifest(Kind::Image).to_json().unwrap();
    for bad in [json.replace("\"kind\"", "\"extra\": 1, \"kind\""), json.replace("\"image\"", "\"web\"")] {
        assert!(matches!(Manifest::from_json(bad.as_bytes()), Err(Error::Json(_))), "{bad}");
    }
}

#[test]
fn schema_must_match() {
    assert_eq!(field(with(|m| m.schema = "wallpaper-rs.wpak/0".into())), "schema");
}

#[test]
fn ids_must_be_reverse_dns() {
    for id in ["org.example.sunset", "io.github.jane-doe.my_wallpaper2"] {
        with(|m| m.id = id.into()).unwrap();
    }
    let long = format!("org.{}", "a".repeat(64));
    let bad =
        ["", "sunset", "Org.Example", "org..sunset", "org.example.", ".org", "org.1st", "org.a b", "org/a.b"];
    for id in bad.into_iter().chain([long.as_str()]) {
        assert_eq!(field(with(|m| m.id = id.into())), "id", "{id:?}");
    }
}

#[test]
fn versions_must_be_semver() {
    for version in ["1.0.0", "0.3.1-beta.2+build.7"] {
        with(|m| m.version = version.into()).unwrap();
    }
    for version in ["", "1", "1.0", "v1.0.0", "01.0.0", "1.0.0.0", "latest"] {
        assert_eq!(field(with(|m| m.version = version.into())), "version", "{version:?}");
    }
}

#[test]
fn licenses_must_be_spdx_expressions() {
    let good =
        ["MIT", "CC-BY-4.0", "GPL-2.0+", "LicenseRef-studio", "(MIT OR Apache-2.0) AND CC0-1.0", "((MIT))"];
    for license in good.into_iter().chain(["GPL-2.0-or-later WITH Classpath-exception-2.0"]) {
        with(|m| m.license = license.into()).unwrap();
    }
    let bad =
        ["", "  ", "MIT AND", "AND MIT", "(MIT", "MIT)", "()", "MIT Apache-2.0", "MIT and GPL-3.0", "M!T"];
    let bad_with =
        ["MIT WITH", "MIT WITH A WITH B", "(MIT) WITH Classpath-exception-2.0", "MIT OR OR GPL-3.0"];
    for license in bad.into_iter().chain(bad_with) {
        assert_eq!(field(with(|m| m.license = license.into())), "license", "{license:?}");
    }
}

#[test]
fn text_fields_are_bounded() {
    assert_eq!(field(with(|m| m.title = " ".into())), "title");
    assert_eq!(field(with(|m| m.title = "t".repeat(129))), "title");
    assert_eq!(field(with(|m| m.authors.push(String::new()))), "authors");
    assert_eq!(field(with(|m| m.tags = vec!["tag".into(); 33])), "tags");
    assert_eq!(field(with(|m| m.tags.push("t".repeat(33)))), "tags");
}
