use super::*;

fn near(a: &Value, b: [f64; 2]) -> bool {
    (a[0].as_f64().unwrap() - b[0]).abs() < 0.02 && (a[1].as_f64().unwrap() - b[1]).abs() < 0.02
}

#[test]
fn handles_place_shapes_and_points_where_the_preview_draws_them() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, "point", json!({ "point": 1, "at": [100, 50], "angle": 90 })).unwrap();
    let around = json!({ "around": { "at": "p1", "count": 4, "radius": 10, "arc": 90 } });
    edit(&mut p, "shape", json!({ "emitter": 0, "shape": around })).unwrap();
    let line = json!({ "between": { "from": [0, 0], "to": [100, 0], "count": 3, "arc": { "amount": 0.5, "side": 1 },
        "taper": { "position": 0.5, "size": 0.2 } } });
    edit(&mut p, "shape", json!({ "emitter": 1, "shape": line })).unwrap();
    let h = handles(&p);
    assert_eq!(h["points"][1]["name"], "p1");
    assert!(near(&h["points"][1]["at"], [100.0, 50.0]), "{}", h["points"][1]);
    let ring = &h["emitters"][0];
    assert_eq!((ring["kind"].clone(), ring["count"].clone()), (json!("around"), json!(4)));
    assert!(near(&ring["places"][0], [100.0, 60.0]) && near(&ring["places"][3], [90.0, 50.0]), "{ring}");
    assert!(near(&ring["ends"][1], [90.0, 50.0]));
    let bow = &h["emitters"][1];
    assert!(near(&bow["bulge"], [50.0, 50.0]), "{bow}");
    let sizes: Vec<f64> =
        bow["places"].as_array().unwrap().iter().map(|p| p["size"].as_f64().unwrap()).collect();
    assert_eq!(sizes, [0.2, 1.0, 0.2]);
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/1/transform", "value": { "position": [1060, 540], "rotation": 90 } } })).unwrap();
    let moved = handles(&p);
    assert!(
        near(&moved["points"][1]["at"], [1060.0 + 540.0 - 50.0, 540.0 - 960.0 + 100.0]),
        "{}",
        moved["points"][1]
    );
    let inverse: Vec<f64> = serde_json::from_value(moved["inverse"].clone()).unwrap();
    let [x, y] =
        [moved["points"][1]["at"][0].as_f64().unwrap(), moved["points"][1]["at"][1].as_f64().unwrap()];
    let back = [inverse[0] * x + inverse[2] * y + inverse[4], inverse[1] * x + inverse[3] * y + inverse[5]];
    assert!((back[0] - 100.0).abs() < 0.05 && (back[1] - 50.0).abs() < 0.05, "{back:?}");
}

#[test]
fn layer_sources_list_only_layers_that_may_feed_the_system_and_show_their_pixels() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "group": { "ids": ["flakes"], "id": "pack" } })).unwrap();
    let h = handles(&p);
    let sources: Vec<&str> = h["sources"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(sources.contains(&"logo") && sources.contains(&"sky"), "{sources:?}");
    assert!(!sources.contains(&"pack") && !sources.contains(&"flakes"), "{sources:?}");
    edit(&mut p, "layer", json!({ "emitter": 0, "id": "logo", "threshold": 0.5 })).unwrap();
    let shown = &handles(&p)["emitters"][0];
    let samples = shown["samples"].as_array().unwrap();
    assert!(!samples.is_empty() && shown.get("approximate").is_none(), "{shown}");
    assert!(
        samples.iter().all(|s| (940.0..961.0).contains(&s[0].as_f64().unwrap())),
        "only the opaque left half"
    );
    assert!(edit(&mut p, "layer", json!({ "emitter": 0, "id": "pack" })).is_err());
}
