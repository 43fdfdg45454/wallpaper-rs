use super::{act, at, project, refused};
use serde_json::json;

#[test]
fn the_details_of_the_package_are_set_with_the_checks_of_the_manifest() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"manifest": {"license": {"license": "CC-BY-4.0"}}})).unwrap();
    assert_eq!(at(&p, "/manifest/license"), "CC-BY-4.0");
    assert!(act(&mut p, json!({"manifest": {"license": {"license": "free as in beer"}}})).is_err());
    act(&mut p, json!({"manifest": {"authors": {"authors": ["Ana", "Luis"]}}})).unwrap();
    act(&mut p, json!({"manifest": {"tags": {"tags": ["sea", "night"]}}})).unwrap();
    act(&mut p, json!({"manifest": {"tags": {"tags": ["calm", "sea"], "add": true}}})).unwrap();
    act(&mut p, json!({"manifest": {"tags": {"tags": ["night"], "remove": true}}})).unwrap();
    assert_eq!(at(&p, "/manifest/tags"), json!(["sea", "calm"]));
    let derived = json!({"kind": "derived", "derived_from": [{"author": "Ana", "license": "CC0-1.0"}]});
    act(&mut p, json!({"manifest": {"origin": derived}})).unwrap();
    assert_eq!(at(&p, "/manifest/origin/derived_from/0/author"), "Ana");
    assert!(act(&mut p, json!({"manifest": {"origin": {"kind": "derived"}}})).is_err());
    assert!(act(&mut p, json!({"manifest": {"display": {"values": {"accent": {"prop": "nope"}}}}})).is_err());
    assert!(act(&mut p, json!({"manifest": {"display": {"values": {"fit": "fit"}}}})).is_err());
    act(&mut p, json!({"manifest": {"display": {"values": {"span": "all-outputs", "accent": "#ff8800"}}}}))
        .unwrap();
    assert_eq!(at(&p, "/manifest/display"), json!({"span": "all-outputs", "accent": "#ff8800"}));
    assert!(refused(&mut p, json!({"manifest": {"display": {"values": {"wobble": 1}}}})).contains("wobble"));
    act(&mut p, json!({"manifest": {"display": {"clear": ["span", "accent"]}}})).unwrap();
    assert_eq!(at(&p, "/manifest/display"), serde_json::Value::Null);
    act(&mut p, json!({"manifest": {"authors": {"authors": []}}})).unwrap();
    assert_eq!(at(&p, "/manifest/authors"), serde_json::Value::Null);
}

#[test]
fn a_property_takes_its_range_default_and_order() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"add_property": {"kind": "slider", "id": "speed"}})).unwrap();
    act(&mut p, json!({"add_property": {"kind": "color", "id": "tint"}})).unwrap();
    act(
        &mut p,
        json!({"property": {"id": "speed", "min": 1, "max": 10, "step": 0.5, "default": 4, "order": 1}}),
    )
    .unwrap();
    let speed = at(&p, "/manifest/properties/speed");
    assert_eq!(
        (&speed["min"], &speed["max"], &speed["step"], &speed["default"]),
        (&json!(1.0), &json!(10.0), &json!(0.5), &json!(4))
    );
    assert!(at(&p, "/manifest/properties/tint/order").as_f64() < speed["order"].as_f64());
    assert!(act(&mut p, json!({"property": {"id": "speed", "default": 40}})).is_err());
    assert!(act(&mut p, json!({"property": {"id": "speed", "min": 20}})).is_err());
    assert!(refused(&mut p, json!({"property": {"id": "tint", "min": 0}})).contains("slider"));
    assert!(refused(&mut p, json!({"property": {"id": "gone", "default": 1}})).contains("gone"));
    act(&mut p, json!({"property": {"id": "tint", "default": "#00ff00"}})).unwrap();
    assert_eq!(at(&p, "/manifest/properties/tint/default"), "#00ff00");
}

#[test]
fn an_asset_is_imported_as_linear_data_or_as_a_sprite_sheet() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let file = root.path().join("sheet.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 255, 255, 255])).save(&file).unwrap();
    act(&mut p, json!({"import": {"file": file, "id": "sheet", "atlas": [2, 4]}})).unwrap();
    act(&mut p, json!({"import": {"file": file, "id": "bumps", "linear": true}})).unwrap();
    assert_eq!(at(&p, "/scene/assets/sheet"), json!({"path": "assets/sheet.png", "atlas": [2, 4]}));
    assert_eq!(at(&p, "/scene/assets/bumps"), json!({"path": "assets/sheet.png", "linear": true}));
    assert!(
        refused(&mut p, json!({"import": {"file": file, "id": "none", "atlas": [0, 4]}})).contains("256")
    );
    assert!(
        refused(&mut p, json!({"import": {"file": file, "id": "tall", "atlas": [300, 1]}})).contains("256")
    );
    let sound = root.path().join("tone.ogg");
    std::fs::write(&sound, b"OggS").unwrap();
    for cut in [json!({"linear": true}), json!({"atlas": [2, 2]})] {
        let mut import = json!({"file": sound, "id": "tone"});
        import.as_object_mut().unwrap().extend(cut.as_object().unwrap().clone());
        assert!(refused(&mut p, json!({ "import": import })).contains("picture"));
    }
}
