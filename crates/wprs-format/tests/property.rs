mod common;

use common::manifest;
use serde_json::json;
use wprs_format::{Choice, Control, Error, Kind, Property, Result, Slider};

fn check(name: &str, property: Property) -> Result<()> {
    let mut manifest = manifest(Kind::Scene);
    manifest.properties.insert(name.into(), property);
    manifest.check()
}

fn slider(min: f64, max: f64, step: f64, default: f64) -> Property {
    Property::new(Control::Slider(Slider::new(min, max, Some(step), default)))
}

fn combo(options: &[&str], default: &str) -> Property {
    let options = options.iter().map(|o| Choice::Plain(json!(o))).collect();
    Property::new(Control::Combo { options, default: json!(default) })
}

fn color(default: &str) -> Property {
    Property::new(Control::Color { default: default.into(), alpha: false })
}

fn text(default: &str) -> Property {
    Property::new(Control::Text { default: default.into(), max_len: None, multiline: false, secret: false })
}

#[test]
fn valid_defaults_pass() {
    let valid = [
        color("#A0b1C2"),
        slider(0.0, 1.0, 0.1, 0.3),
        slider(-5.0, 5.0, 1.0, -5.0),
        slider(0.0, 1e9, 0.001, 123456.789),
        Property::new(Control::Bool { default: true }),
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
            matches!(check(name, Property::new(Control::Bool { default: true })), Err(Error::Invalid { field, .. }) if field == format!("properties.{name}")),
            "{name:?}"
        );
    }
}

#[test]
fn presets_keep_whole_numbers_on_sliders_and_colors_in_any_case() {
    let props = serde_json::from_value(json!({
        "speed": { "type": "slider", "min": 0.5, "max": 4, "decimals": 2, "default": 1 },
        "count": { "type": "slider", "min": 0, "max": 9.5, "default": 2 },
        "tint": { "type": "color", "default": "#000000" }
    }))
    .unwrap();
    let preset: wprs_format::PresetFile = serde_json::from_value(json!({
        "base": { "id": "org.example.base", "version": "^1" },
        "values": { "speed": 1, "count": 5, "tint": "#00FF00" }
    }))
    .unwrap();
    let chosen = wprs_format::values::chosen(&preset, &props);
    assert!(chosen.warnings.is_empty(), "{:?}", chosen.warnings);
    assert_eq!(chosen.values["tint"], json!("#00ff00"));
    assert_eq!((chosen.values["speed"].as_f64(), chosen.values["count"].as_f64()), (Some(1.0), Some(5.0)));
    let far: wprs_format::PresetFile = serde_json::from_value(json!({
        "base": { "id": "org.example.base", "version": "^1" },
        "values": { "speed": 9, "count": -1 }
    }))
    .unwrap();
    let chosen = wprs_format::values::chosen(&far, &props);
    assert_eq!(chosen.warnings, ["count is out of range", "speed is out of range"]);
    assert!(chosen.values.is_empty());
}
