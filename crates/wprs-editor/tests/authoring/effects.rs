use super::{act, at, project, refused};
use serde_json::json;

fn effects(edit: serde_json::Value) -> serde_json::Value {
    json!({ "effects": edit })
}

#[test]
fn an_effect_starts_the_stack_of_a_layer_that_has_none() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let path = wprs_editor::layers::find(&p.doc, "card").unwrap();
    assert_eq!(at(&p, &format!("{path}/effects")), serde_json::Value::Null);
    let add =
        json!({"add": {"layer": "card", "effect": "vignette", "params": {"intensity": 0.4}, "name": "edge"}});
    act(&mut p, effects(add)).unwrap();
    act(&mut p, effects(json!({"add": {"layer": "card", "effect": "film_grain", "at": 0}}))).unwrap();
    let stack = at(&p, &format!("{path}/effects"));
    assert_eq!(stack[0]["effect"], "film_grain");
    assert_eq!(stack[1], json!({"effect": "vignette", "name": "edge", "params": {"intensity": 0.4}}));
    act(&mut p, json!({"undo": {}})).unwrap();
    act(&mut p, json!({"undo": {}})).unwrap();
    assert_eq!(at(&p, &format!("{path}/effects")), serde_json::Value::Null);
}

#[test]
fn unknown_effects_parameters_and_values_out_of_range_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let unknown = refused(&mut p, effects(json!({"add": {"layer": "card", "effect": "sparkle_dust"}})));
    assert!(unknown.contains("sparkle_dust"), "{unknown}");
    let param = json!({"add": {"layer": "card", "effect": "vignette", "params": {"wobble": 1}}});
    assert!(refused(&mut p, effects(param)).contains("wobble"));
    let range = json!({"add": {"layer": "card", "effect": "vignette", "params": {"intensity": 99}}});
    assert!(refused(&mut p, effects(range)).contains("intensity"));
    assert!(
        refused(&mut p, effects(json!({"add": {"layer": "nope", "effect": "vignette"}}))).contains("nope")
    );
    assert!(refused(&mut p, effects(json!({"set": {"layer": "card", "index": 0}}))).contains("card"));
    let far = json!({"add": {"layer": "card", "effect": "vignette", "at": 3}});
    assert!(refused(&mut p, effects(far)).contains('3'));
}

#[test]
fn an_effect_changes_moves_and_leaves_with_its_parameters() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let path = wprs_editor::layers::find(&p.doc, "card").unwrap();
    act(&mut p, effects(json!({"add": {"layer": "card", "effect": "vignette"}}))).unwrap();
    act(&mut p, effects(json!({"add": {"layer": "card", "effect": "film_grain"}}))).unwrap();
    let set = json!({"set": {"layer": "card", "index": 0, "params": {"intensity": 0.7}, "enabled": false, "optional": true}});
    act(&mut p, effects(set)).unwrap();
    let first = at(&p, &format!("{path}/effects/0"));
    assert_eq!(
        (&first["params"]["intensity"], &first["enabled"], &first["optional"]),
        (&json!(0.7), &json!(false), &json!(true))
    );
    act(&mut p, effects(json!({"set": {"layer": "card", "index": 0, "params": {"intensity": null}}})))
        .unwrap();
    assert!(at(&p, &format!("{path}/effects/0")).get("params").is_none());
    act(&mut p, effects(json!({"move": {"layer": "card", "index": 0, "to": 1}}))).unwrap();
    assert_eq!(at(&p, &format!("{path}/effects/1/effect")), "vignette");
    assert!(refused(&mut p, effects(json!({"move": {"layer": "card", "index": 0, "to": 2}}))).contains('2'));
    act(&mut p, effects(json!({"remove": {"layer": "card", "index": 0}}))).unwrap();
    act(&mut p, effects(json!({"remove": {"layer": "card", "index": 0}}))).unwrap();
    assert!(at(&p, &path).get("effects").is_none());
}

#[test]
fn a_mask_limits_an_effect_to_a_shape_a_layer_or_a_picture() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let path = wprs_editor::layers::find(&p.doc, "card").unwrap();
    act(&mut p, json!({"add_layer": {"kind": "solid", "id": "hole"}})).unwrap();
    act(&mut p, effects(json!({"add": {"layer": "card", "effect": "vignette"}}))).unwrap();
    let shape = json!({"ellipse": {"size": [200, 100]}});
    let mask = json!({"mask": {"layer": "card", "index": 0, "shape": shape, "feather": 12, "invert": true}});
    act(&mut p, effects(mask)).unwrap();
    let pointer = format!("{path}/effects/0/mask");
    assert_eq!(at(&p, &pointer), json!({"shape": shape, "feather": 12.0, "invert": true}));
    act(
        &mut p,
        effects(json!({"mask": {"layer": "card", "index": 0, "from": "hole", "channel": "luminance"}})),
    )
    .unwrap();
    assert_eq!(
        at(&p, &pointer),
        json!({"layer": "hole", "feather": 12.0, "invert": true, "channel": "luminance"})
    );
    let selfish = json!({"mask": {"layer": "card", "index": 0, "from": "card"}});
    refused(&mut p, effects(selfish));
    assert!(
        refused(&mut p, effects(json!({"mask": {"layer": "card", "index": 0, "strength": 3}})))
            .contains("strength")
    );
    act(&mut p, effects(json!({"mask": {"layer": "card", "index": 0, "off": true}}))).unwrap();
    assert_eq!(at(&p, &pointer), serde_json::Value::Null);
    let empty = refused(&mut p, effects(json!({"mask": {"layer": "card", "index": 0, "invert": true}})));
    assert!(empty.contains("mask"), "{empty}");
}

#[test]
fn adding_to_the_end_of_a_missing_list_creates_it() {
    let mut doc = json!({"a": {}});
    let undo =
        wprs_editor::ops::apply(&mut doc, &[wprs_editor::Op::Add { path: "/a/b/-".into(), value: json!(1) }])
            .unwrap();
    assert_eq!(doc, json!({"a": {"b": [1]}}));
    wprs_editor::ops::apply(&mut doc, &undo).unwrap();
    assert_eq!(doc, json!({"a": {}}));
    let deep =
        wprs_editor::ops::apply(&mut doc, &[wprs_editor::Op::Add { path: "/x/b/-".into(), value: json!(1) }]);
    assert!(deep.is_err());
    assert!(
        wprs_editor::ops::apply(&mut doc, &[wprs_editor::Op::Add { path: "/a/c/3".into(), value: json!(1) }])
            .is_err()
    );
}
