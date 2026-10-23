use serde_json::{Value, json};
use wprs_editor::actions::{Action, plan, visible};
use wprs_editor::check::check;
use wprs_editor::inspect::fields;
use wprs_editor::keys::value_at;
use wprs_editor::ops::apply;
use wprs_editor::starters::starter;
use wprs_editor::{ACTIONS, STARTERS};

fn act(doc: &mut Value, action: Value) {
    let action: Action = serde_json::from_value(action).unwrap();
    assert!(ACTIONS.contains(&action.name().as_str()));
    apply(doc, &plan(doc, &action).unwrap()).unwrap();
}

fn ids(doc: &Value, list: &str) -> Vec<String> {
    doc.pointer(list)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn every_starting_point_is_a_valid_scene() {
    for name in STARTERS {
        let doc = starter(name, "org.example.test", "Test", "en").unwrap();
        assert_eq!(check(&doc), [], "{name}");
        let manifest: wprs_format::Manifest = serde_json::from_value(doc["manifest"].clone()).unwrap();
        assert!(manifest.v2() && manifest.content.is_some() && manifest.photosensitive == Some(false));
        let pointer = name == "parallax";
        assert_eq!(manifest.inputs.len(), usize::from(pointer), "{name}");
        assert_eq!(manifest.features() == ["inputs.pointer"], pointer, "{name}");
    }
    assert!(starter("nope", "org.example.test", "Test", "en").is_err());
}

#[test]
fn layer_actions_keep_the_scene_valid() {
    let mut doc = starter("parallax", "org.example.test", "Test", "en").unwrap();
    act(&mut doc, json!({"add_layer": {"kind": "text"}}));
    act(&mut doc, json!({"add_layer": {"kind": "particles", "at": 0}}));
    assert_eq!(doc["scene"]["particles"]["particles"]["capacity"], 2000);
    act(&mut doc, json!({"duplicate_layer": {"id": "sun"}}));
    act(&mut doc, json!({"group": {"ids": ["near", "far"], "id": "hills"}}));
    assert_eq!(
        ids(&doc, "/scene/layers"),
        ["particles", "sky", "sun", "sun_copy", "hills", "middle", "text"]
    );
    assert_eq!(ids(&doc, "/scene/layers/4/layers"), ["far", "near"]);
    act(&mut doc, json!({"move_layer": {"id": "middle", "to": 0, "into": "hills"}}));
    act(&mut doc, json!({"move_layer": {"id": "text", "to": 0}}));
    assert_eq!(ids(&doc, "/scene/layers/5/layers"), ["middle", "far", "near"]);
    act(&mut doc, json!({"ungroup": {"id": "hills"}}));
    act(&mut doc, json!({"remove_layer": {"id": "sun_copy"}}));
    assert_eq!(ids(&doc, "/scene/layers"), ["text", "particles", "sky", "sun", "middle", "far", "near"]);
    act(&mut doc, json!({"hide": {"id": "sky", "hidden": true}}));
    assert_eq!(doc["scene"]["layers"][2]["visible"], false);
    act(&mut doc, json!({"hide": {"id": "sky", "hidden": false}}));
    assert!(doc["scene"]["layers"][2].get("visible").is_none());
    act(&mut doc, json!({"lock": {"id": "sky", "locked": true}}));
    act(&mut doc, json!({"solo": {"id": "sun"}}));
    assert_eq!(doc["scene"]["ext"]["editor"], json!({"locked": ["sky"], "solo": "sun"}));
    let shown = visible(&doc);
    assert_eq!((&shown["layers"][3].get("visible"), &shown["layers"][2]["visible"]), (&None, &json!(false)));
    act(&mut doc, json!({"set": {"pointer": "/scene/layers/3/opacity", "value": 0.5}}));
    assert_eq!(check(&doc), []);
    let bad: Action = serde_json::from_value(json!({"group": {"ids": ["sky", "nope"]}})).unwrap();
    assert!(plan(&doc, &bad).is_err());
    act(&mut doc, json!({"lock": {"id": "sun", "locked": true}}));
    act(&mut doc, json!({"remove_layer": {"id": "sun"}}));
    assert_eq!(doc["scene"]["ext"]["editor"], json!({"locked": ["sky"]}));
    act(&mut doc, json!({"set": {"pointer": "/scene/layers/0/visible", "value": {"prop": "shown"}}}));
    let unhide: Action = serde_json::from_value(json!({"hide": {"id": "text", "hidden": false}})).unwrap();
    assert_eq!(plan(&doc, &unhide).unwrap(), []);
    act(&mut doc, json!({"add_layer": {"kind": "text", "name": "Título"}}));
    act(&mut doc, json!({"add_layer": {"kind": "solid", "name": "Color sólido"}}));
    act(&mut doc, json!({"add_layer": {"kind": "shape", "name": "形状"}}));
    act(&mut doc, json!({"add_layer": {"kind": "text", "name": "Título"}}));
    assert_eq!(ids(&doc, "/scene/layers")[6..], ["titulo", "color_solido", "shape", "titulo_2"]);
    assert_eq!(doc["scene"]["layers"][6]["text"], "Título");
    assert!(check(&doc).iter().all(|p| p.pointer == "/scene/layers/0/visible"));
}

#[test]
fn keyframes_animate_values() {
    let mut doc = starter("clock", "org.example.test", "Test", "en").unwrap();
    act(&mut doc, json!({"add": {"pointer": "/scene/timelines", "value": [{"id": "intro", "duration": 2}]}}));
    let opacity = "/scene/layers/1/opacity";
    act(&mut doc, json!({"keyframe": {"pointer": opacity, "time": 2, "value": 1}}));
    act(
        &mut doc,
        json!({"keyframe": {"pointer": opacity, "time": 0, "value": 0, "ease": "in_out_sine", "timeline": "intro"}}),
    );
    let value = doc.pointer(opacity).unwrap().clone();
    assert_eq!(value, json!({"keys": [[0.0, 0, "in_out_sine"], [2.0, 1]], "timeline": "intro"}));
    assert!((value_at(&value, 1.0).as_f64().unwrap() - 0.5).abs() < 1e-6);
    act(&mut doc, json!({"keyframe": {"pointer": opacity, "time": 1}}));
    let middle = &doc.pointer(opacity).unwrap()["keys"][1];
    assert!(middle[0] == 1.0 && (middle[1].as_f64().unwrap() - 0.5).abs() < 1e-6);
    act(&mut doc, json!({"keyframe": {"pointer": opacity, "time": 1, "remove": true}}));
    act(&mut doc, json!({"keyframe": {"pointer": opacity, "time": 0, "remove": true}}));
    act(&mut doc, json!({"keyframe": {"pointer": opacity, "time": 2, "remove": true}}));
    assert_eq!(doc.pointer(opacity), Some(&json!(1)));
    assert_eq!(check(&doc), []);
    let bound: Action =
        serde_json::from_value(json!({"keyframe": {"pointer": "/scene/layers/1/text", "time": 0}})).unwrap();
    assert!(plan(&doc, &bound).is_err(), "a clock text follows another source");
}

#[test]
fn the_inspector_follows_the_schema() {
    let mut doc = starter("parallax", "org.example.test", "Test", "en").unwrap();
    act(&mut doc, json!({"set": {"pointer": "/scene/layers/1/effects", "value": [{"effect": "vignette"}]}}));
    let shown = fields(&doc, "/scene/layers/1");
    let field = |key: &str| shown.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{key}"));
    assert_eq!((field("opacity").widget.as_str(), field("opacity").bindable), ("number", true));
    assert_eq!(field("opacity").range, json!([0, 1]));
    assert_eq!((field("blend").widget.as_str(), field("blend").options.len()), ("enum", 17));
    assert_eq!((field("position").widget.as_str(), field("position").unit.as_str()), ("vec2", "px"));
    assert_eq!(field("fill").widget, "color");
    assert_eq!(field("shape").widget, "variant");
    assert!(
        field("shape").options.contains(&json!("star"))
            && field("size").pointer.ends_with("/shape/ellipse/size")
    );
    let effect = fields(&doc, "/scene/layers/1/effects/0");
    let id = effect.iter().find(|f| f.key == "effect").unwrap();
    assert!(id.widget == "ref:effect" && id.options.contains(&json!("vignette")) && id.options.len() > 30);
    let clock = fields(&doc, "/scene/clock");
    let fps = clock.iter().find(|f| f.key == "max_fps").unwrap();
    assert_eq!(
        (fps.widget.as_str(), &fps.options, &fps.value),
        ("integer", &vec![json!("display")], &json!("display"))
    );
    let intensity = effect.iter().find(|f| f.key == "intensity").unwrap();
    assert_eq!(intensity.range, json!([0.0, 1.0]));
    assert!((intensity.default.as_f64().unwrap() - 0.35).abs() < 1e-6);
    act(&mut doc, json!({"add_layer": {"kind": "particles"}}));
    let system = fields(&doc, "/scene/particles/particles");
    assert!(system.iter().any(|f| f.key == "emitters" && f.widget == "list" && f.sample.is_object()));
    assert!(system.iter().any(|f| f.key == "size_over_life" && f.widget == "curve"));
    let prop = fields(
        &starter("particles", "org.example.test", "Test", "en").unwrap(),
        "/manifest/properties/density",
    );
    assert!(prop.iter().any(|f| f.key == "max" && f.widget == "number"));
    let layer = fields(&doc, "/scene/layers/5");
    assert!(
        layer
            .iter()
            .any(|f| f.key == "system" && f.widget == "ref:system" && f.options == [json!("particles")])
    );
    assert!(layer.iter().any(|f| f.key == "system" && f.required));
    assert!(layer.iter().any(|f| f.key == "opacity" && !f.required));
    act(
        &mut doc,
        json!({"set": {"pointer": "/scene/layers/1/opacity", "value": {"audio": "bass", "smooth": 0.2}}}),
    );
    let binding = fields(&doc, "/scene/layers/1/opacity");
    let field = |key: &str| binding.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{key}"));
    assert_eq!((field("audio").widget.as_str(), &field("audio").value), ("enum", &json!("bass")));
    assert_eq!((field("timeline").widget.as_str(), field("smooth").set), ("ref:timeline", true));
    let label = prop.iter().find(|f| f.key == "label").unwrap();
    assert_eq!(label.value, json!({"en": "Density", "es": "Densidad"}));
}
