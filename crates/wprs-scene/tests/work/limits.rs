use super::*;

#[test]
fn targets_shader_size_and_expressions_have_limits() {
    let targets: serde_json::Map<String, Json> =
        (0..=MAX_TARGETS).map(|k| (format!("t{k}"), json!({ "scale": 1 }))).collect();
    let fault = effect(|d| d["targets"] = Json::Object(targets), "").unwrap_err();
    assert!(fault.contains(&format!("more than {MAX_TARGETS} targets")), "{fault}");
    let padded = format!("{}{}", shader(""), " ".repeat(MAX_WGSL));
    let d = json!({ "id": "w", "cost": {}, "passes": [{ "entry": "fs_w" }] }).to_string();
    let fault = Descriptor::packaged("w", &d, &padded).unwrap_err();
    assert!(fault.contains(&format!("larger than {} KiB", MAX_WGSL >> 10)), "{fault}");
    let long = "c=c*c*c*c*c*c*c*c;".repeat(MAX_SHADER_NODES / 14);
    let fault = effect(|_| (), &long).unwrap_err();
    assert!(fault.contains(&format!("more than {MAX_SHADER_NODES} expressions")), "{fault}");
    for cost in [json!({ "taps": -1 }), json!({ "scale": -0.5 }), json!({ "passes": 99 })] {
        let fault = effect(|d| d["cost"] = cost.clone(), "").unwrap_err();
        assert!(fault.contains("the cost needs finite numbers"), "{cost}: {fault}");
    }
}

#[test]
fn passes_swaps_and_compose_have_limits() {
    let targets = json!({ "a": { "scale": 0.5 }, "b": { "scale": 0.5 }, "c": { "scale": 1 } });
    let many: Vec<Json> = (0..=MAX_PASSES).map(|_| json!({ "entry": "fs_w" })).collect();
    let cases = [
        (json!({ "passes": many }), format!("more than {MAX_PASSES} passes")),
        (
            json!({ "targets": targets, "passes": [{ "swap": ["a", "c"] }, { "entry": "fs_w" }] }),
            "not two of the same".into(),
        ),
        (
            json!({ "targets": targets, "passes": [{ "swap": ["a", "a"] }, { "entry": "fs_w" }] }),
            "not two of the same".into(),
        ),
        (
            json!({ "targets": targets, "passes": [{ "swap": ["a", "x"] }, { "entry": "fs_w" }] }),
            "not two of the same".into(),
        ),
        (
            json!({ "targets": targets, "passes": [{ "entry": "fs_w", "swap": ["a", "b"] }] }),
            "needs either an entry, a copy or a swap".into(),
        ),
    ];
    for (patch, problem) in cases {
        let fault = effect(|d| d.as_object_mut().unwrap().extend(patch.as_object().unwrap().clone()), "");
        let fault = fault.err().unwrap_or_else(|| panic!("{patch} was accepted"));
        assert!(fault.contains(&problem), "{patch}: {fault}");
    }
    let swapped = json!({ "targets": targets, "compose": true, "passes": [{ "swap": ["a", "b"] }, { "entry": "fs_w" }] });
    let d = effect(|d| d.as_object_mut().unwrap().extend(swapped.as_object().unwrap().clone()), "").unwrap();
    assert!(d.compose && d.below() && d.newer() && d.key != 0);
}

#[test]
fn package_costs_count_their_passes_and_heavy_scenes_warn() {
    let d =
        effect(|d| d["passes"] = json!([{ "entry": "fs_w" }, { "entry": "fs_w", "vertex": "fs_w2" }]), "");
    assert!(d.is_err());
    let cheap = effect(|d| d["passes"] = json!(vec![json!({ "entry": "fs_w" }); 6]), "").unwrap();
    assert_eq!(cheap.units(), 6.0);
    assert_eq!(cheap.class(), Class::Medium);
    let looped = "for (var i = 0u; i < 32u; i++) { for (var j = 0u; j < 32u; j++) { c += tap(in.uv); } }";
    let sly = effect(|d| d["cost"] = json!({ "taps": 0.01 }), looped).unwrap();
    assert!(sly.units() >= 256.0 && sly.class() == Class::Heavy, "{}", sly.units());
    let solid = |effects: Json| json!({ "id": "l", "type": "solid", "color": "#ffffff", "effects": effects });
    let scene = |effects: Json| {
        let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "layers": [solid(effects)] });
        Scene::from_json(&text.to_string()).unwrap().cost(Quality::Q2)
    };
    let tint = scene(json!([{ "effect": "tint" }]));
    let json = json!({ "id": "under_cost", "passes": [{ "entry": "fs_under_cost", "inputs": ["previous", "backdrop"] }] });
    register(Descriptor::parse(&json.to_string(), "").unwrap()).unwrap();
    let read = scene(json!([{ "effect": "tint" }, { "effect": "under_cost" }]));
    assert!((read - tint - 3.0).abs() < 1e-3, "{tint} {read}");
    let masked = |mask: Json| {
        let hidden = json!({ "id": "h", "type": "solid", "color": "#ffffff", "visible": false });
        let a = json!({ "id": "a", "type": "solid", "color": "#ffffff", "mask": mask });
        let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "layers": [a, hidden] });
        Scene::from_json(&text.to_string()).unwrap().cost(Quality::Q2)
    };
    let (shaped, layered) =
        (masked(json!({ "shape": { "ellipse": { "size": [8, 8] } } })), masked(json!({ "layer": "h" })));
    assert!((layered - shaped - 1.0).abs() < 1e-3, "{shaped} {layered}");
    let layers: Vec<Json> = (0..12)
        .map(|i| {
            json!({ "id": format!("l{i}"), "type": "solid", "color": "#ffffff",
        "effects": [{ "effect": "blur", "params": { "radius": 32 } }] })
        })
        .collect();
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "layers": layers });
    let heavy = Scene::from_json(&text.to_string()).unwrap();
    assert!(heavy.heavy().is_some_and(|m| m.contains("older integrated and phone GPUs")));
    assert!(Scene::new([64, 32]).heavy().is_none());
}
