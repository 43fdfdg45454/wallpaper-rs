mod common;

use common::manifest;
use wprs_format::{Error, Kind, Property, Result};

fn check(name: &str, property: Property) -> Result<()> {
    let mut manifest = manifest(Kind::Scene);
    manifest.properties.insert(name.into(), property);
    manifest.check()
}

fn slider(min: f64, max: f64, step: f64, default: f64) -> Property {
    Property::Slider { min, max, step, default }
}

fn combo(options: &[&str], default: &str) -> Property {
    Property::Combo { options: options.iter().map(|o| o.to_string()).collect(), default: default.into() }
}

fn color(default: &str) -> Property {
    Property::Color { default: default.into() }
}

fn text(default: &str) -> Property {
    Property::Text { default: default.into() }
}

#[test]
fn valid_defaults_pass() {
    let valid = [
        color("#A0b1C2"),
        slider(0.0, 1.0, 0.1, 0.3),
        slider(-5.0, 5.0, 1.0, -5.0),
        slider(0.0, 1e9, 0.001, 123456.789),
        Property::Bool { default: true },
        combo(&["calm", "storm"], "storm"),
        text(""),
        text(&"é".repeat(1024)),
    ];
    for property in valid {
        check("value", property.clone()).unwrap_or_else(|e| panic!("{property:?}: {e}"));
    }
}

#[test]
fn invalid_defaults_are_rejected() {
    let invalid = [
        color("red"),
        color("#12345"),
        color("#12345g"),
        color("#1234567"),
        color("123456"),
        slider(0.0, 1.0, 0.1, 1.5),
        slider(0.0, 1.0, 0.1, -0.1),
        slider(0.0, 1.0, 0.3, 0.5),
        slider(1.0, 1.0, 0.1, 1.0),
        slider(2.0, 1.0, 0.1, 1.5),
        slider(0.0, 1.0, 0.0, 0.5),
        slider(0.0, 1.0, -0.1, 0.5),
        slider(0.0, 1.0, 2.0, 0.0),
        slider(0.0, f64::NAN, 0.1, 0.0),
        slider(0.0, f64::INFINITY, 0.1, 0.0),
        combo(&[], ""),
        combo(&["a", "a"], "a"),
        combo(&["a", ""], "a"),
        combo(&["a", "b"], "c"),
        text(&"x".repeat(1025)),
    ];
    for property in invalid {
        match check("value", property.clone()) {
            Err(Error::Invalid { field, .. }) => assert_eq!(field, "properties.value"),
            other => panic!("{property:?} was accepted: {other:?}"),
        }
    }
}

#[test]
fn property_names_must_be_identifiers() {
    for name in ["", "Bad", "two words", "1st", "dot.ted", &"n".repeat(64)] {
        assert!(
            matches!(check(name, Property::Bool { default: true }), Err(Error::Invalid { field, .. }) if field == format!("properties.{name}")),
            "{name:?}"
        );
    }
}
