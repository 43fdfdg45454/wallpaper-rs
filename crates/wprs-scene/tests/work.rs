use serde_json::{Value as Json, json};
use wprs_scene::*;

fn shader(body: &str) -> String {
    format!(
        "override taps: u32 = 4u;\noverride neg: i32 = -1;\nfn helper(p: ptr<function, u32>) {{ *p = 1u; }}\n@fragment\nfn fs_w(in: VertexOut) -> @location(0) vec4<f32> {{\n    var c = tap(in.uv);\n{body}\n    return c;\n}}"
    )
}

fn effect(change: impl FnOnce(&mut Json), body: &str) -> Result<Descriptor, String> {
    let mut d = json!({ "id": "w", "cost": {}, "passes": [{ "entry": "fs_w" }],
        "options": [{ "id": "taps", "values": [4, 16], "default": 4 }] });
    change(&mut d);
    Descriptor::packaged("w", &d.to_string(), &shader(body))
}

#[test]
fn counted_loops_pass_and_unbounded_work_is_refused() {
    let fine = [
        "for (var i = 0u; i < 8u; i++) { c += tap(in.uv + f32(i)); }",
        "for (var i = 0; i <= 1023; i += 1) { c *= 0.99; }",
        "for (var i = 0u; i < taps; i++) { c *= 0.9; }",
        "for (var i = 0u; i < min(u32(param(0u).x), 64u); i++) { c *= 0.9; }",
        "for (var i = 16; i > 0; i--) { c *= 0.9; }",
        "for (var i = 0u; i < 32u; i++) { for (var j = 0u; j < 32u; j++) { c *= 0.99; } }",
        "for (var i = 10u; i >= 2u; i -= 2u) { c *= 0.9; }",
        "for (var i = 4294967290u; i < 4294967295u; i++) { c *= 0.9; }",
        "let m = 8i; for (var i = 0u; i < u32(m); i++) { c *= 0.9; }",
        "for (var i = 0u; i < min(u32(neg), 64u); i++) { c *= 0.9; }",
    ];
    for body in fine {
        effect(|_| (), body).unwrap_or_else(|e| panic!("{body}: {e}"));
    }
    let refused = [
        ("var i = 0u; loop { if i > 4u { break; } i++; }", "a counter that a literal limits"),
        ("loop { if c.a < -1.0 { break; } }", "a counter that a literal limits"),
        ("for (var i = 0u; i < u32(param(0u).x); i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 0u; i < 8u; i++) { i = 0u; }", "a counter that a literal limits"),
        ("for (var i = 0u; i < 4096u; i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("var i = 0u; loop { i++; continuing { break if i > 4u; } }", "a counter that a literal limits"),
        ("for (var i = 10u; i >= 0u; i--) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 10u; i > 0u; i -= 3u) { c *= 0.9; }", "a counter that a literal limits"),
        (
            "for (var x = 16777216.0; x < 16777300.0; x += 1.0) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        (
            "for (var i = 0u; i < 16u; i += min(u32(param(0u).x), 8u)) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        ("for (var i = 0u; i < 16u; i += taps) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 2147483640; i <= 2147483647; i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 4294967290u; i <= 4294967295u; i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 5; u32(i) >= 0u; i--) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 0u; i < bitcast<u32>(1.0f); i++) { c *= 0.9; }", "a counter that a literal limits"),
        (
            "for (var i = 0u; i < 16u; i += bitcast<u32>(1.0f)) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        (
            "for (var i = 4294967000u; i < 4294967295u; i += bitcast<u32>(1.0f)) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        ("let m = -1i; for (var i = 0u; i <= u32(m); i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 0u; i <= u32(neg); i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i = 0u; i <= bitcast<u32>(neg); i++) { c *= 0.9; }", "a counter that a literal limits"),
        ("for (var i: u32 = u32(neg); i > 0u; i--) { c *= 0.9; }", "a counter that a literal limits"),
        (
            "for (var i: u32 = bitcast<u32>(neg); i > 0u; i--) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        (
            "let m = 4294967200u; for (var i = 4294967290u; i <= u32(f32(m)); i++) { c *= 0.9; }",
            "a counter that a literal limits",
        ),
        ("var k = 0u; helper(&k); c *= f32(k);", "pointer to a local variable"),
        (
            "for (var i = 0u; i < 512u; i++) { for (var j = 0u; j < 512u; j++) { c *= 0.99; } }",
            "operations a pixel",
        ),
    ];
    for (body, problem) in refused {
        let fault = effect(|_| (), body).err().unwrap_or_else(|| panic!("{body} was accepted"));
        assert!(fault.contains(problem), "{body}: {fault}");
    }
    let wide = effect(
        |d| d["options"][0]["values"] = json!([4, 2048]),
        "for (var i = 0u; i < taps; i++) { c *= 0.9; }",
    );
    assert!(wide.unwrap_err().contains("a counter that a literal limits"));
    let negative = |d: &mut Json| {
        d["options"].as_array_mut().unwrap().push(json!({ "id": "neg", "values": [-1, -2], "default": -1 }))
    };
    for body in [
        "for (var i: u32 = u32(neg); i > 0u; i--) { c *= 0.9; }",
        "for (var i = 0u; i <= u32(neg); i++) { c *= 0.9; }",
    ] {
        assert!(effect(negative, body).unwrap_err().contains("a counter that a literal limits"), "{body}");
    }
    let huge = effect(|d| d["options"][0]["values"] = json!([4, -1]), "");
    assert!(huge.unwrap_err().contains("values its override cannot hold"));
    let stepped = effect(
        |d| d["options"][0]["values"] = json!([0, 4]),
        "for (var i = 0u; i < 16u; i += taps) { for (var j = 0u; j < 1000u; j += taps) { c *= 0.9; } }",
    );
    assert!(stepped.unwrap_err().contains("a counter that a literal limits"));
}

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
