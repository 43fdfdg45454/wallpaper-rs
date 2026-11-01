use super::*;

#[test]
fn package_effects_compile_against_the_contract_and_report_what_they_read() {
    let d = packaged(|_| (), WGSL).unwrap_or_else(|e| panic!("{e}"));
    assert!(d.uses.contains(&Use::Time) && d.uses.contains(&Use::Audio) && !d.uses.contains(&Use::Cursor));
    let plain =
        |b: &&Descriptor| newer_params(&b.id).next().is_none() && b.params.iter().all(|p| p.label.is_empty());
    let basic = [
        ParamKind::Float,
        ParamKind::Vec2,
        ParamKind::Color,
        ParamKind::Bool,
        ParamKind::Texture,
        ParamKind::Lut,
    ];
    let mut first =
        builtin().iter().filter(plain).filter(|b| b.params.iter().all(|p| basic.contains(&p.kind)));
    assert!(d.newer() && first.all(|b| !b.newer()));
    assert_eq!(d.params[2].slot(&Param::Text("linear_burn".into()))[0], Blend::LinearBurn as u8 as f32);
    assert!(d.params[1].accepts(&Param::Number(1.0)) && !d.params[1].accepts(&Param::Number(2.0)));
    assert!(!d.params[0].accepts(&Param::Number(1.5)) && d.params[3].accepts(&Param::Vec2([0.2, 0.8])));
    let broken = [
        (json!({ "id": "other" }), WGSL, "declares the id other, expected ripple"),
        (json!({ "id": "blur" }), WGSL, "blur is the name of a built-in effect"),
        (json!({ "cost": null }), WGSL, "needs a cost"),
        (json!({}), "fn broken(", "expected"),
        (json!({}), &WGSL.replace("fn fs_ripple(", "fn fs_other("), "has no entry point fs_ripple"),
        (json!({}), &WGSL.replace("override taps", "const taps"), "declares no override taps"),
        (
            json!({}),
            &format!("{WGSL}\n@group(2) @binding(0) var extra: texture_2d<f32>;"),
            "bindings of its own",
        ),
        (json!({ "targets": { "history": { "scale": 3 } } }), WGSL, "a scale outside (0, 2]"),
        (
            json!({ "options": [{ "id": "taps", "values": [0.5], "default": 0.5 }] }),
            WGSL,
            "whole numbers or flags",
        ),
        (
            json!({ "options": [{ "id": "taps", "values": [4], "default": 8 }] }),
            WGSL,
            "not one of its values",
        ),
        (json!({ "options": [{ "id": "detail", "values": [4], "default": 4 }] }), WGSL, "not a unique id"),
        (
            json!({ "options": [{ "id": "taps", "values": [4], "default": 4, "when_bound": "detail" }] }),
            WGSL,
            "no texture",
        ),
    ];
    for (patch, wgsl, problem) in broken {
        let apply = |d: &mut Json| {
            for (k, v) in patch.as_object().unwrap() {
                match v.is_null() {
                    true => drop(d.as_object_mut().unwrap().remove(k)),
                    false => d[k] = v.clone(),
                }
            }
        };
        let fault = packaged(apply, wgsl);
        let fault = fault.err().unwrap_or_else(|| panic!("{patch} was accepted"));
        assert!(fault.contains(problem), "{patch}: {fault}");
    }
}

#[test]
fn descriptors_check_their_new_fields() {
    let pass = |p: Json| json!({ "id": "probe", "passes": [p], "cost": {} });
    let params = |p: Json| json!({ "id": "probe", "params": [p], "passes": [{ "entry": "fs_a" }] });
    let many: Vec<Json> =
        (0..25).map(|i| json!({ "id": format!("p{i}"), "type": "float", "default": 0 })).collect();
    let cases = [
        (pass(json!({ "entry": "fs_a", "copy": "previous" })), "needs either an entry, a copy or a swap"),
        (pass(json!({ "entry": "fs_a", "target": "nowhere" })), "writes an unknown target"),
        (pass(json!({ "entry": "fs_a", "grid": [8, 8] })), "needs a vertex entry and a grid"),
        (pass(json!({ "entry": "fs_a", "vertex": "vs_a", "grid": [65, 8] })), "a grid of 1 to 64"),
        (pass(json!({ "entry": "fs_a", "inputs": vec!["input"; 10] })), "more than 9 inputs"),
        (pass(json!({ "entry": "fs_a", "inputs": ["target:nowhere"] })), "unknown input target:nowhere"),
        (params(json!({ "id": "k", "type": "enum", "default": 0 })), "needs 1 to 64 values"),
        (params(json!({ "id": "k", "type": "float", "default": 0, "channels": 3 })), "is no mask"),
        (
            params(json!({ "id": "k", "type": "mask", "default": "", "channels": 2 })),
            "channels other than 1, 3 or 4",
        ),
        (params(json!({ "id": "k", "type": "blend", "default": "sparkle" })), "default of k does not fit"),
        (
            params(json!({ "id": "k", "type": "float", "default": 0, "label": { "en": "x".repeat(300) } })),
            "256",
        ),
        (
            json!({ "id": "probe", "params": many, "passes": [{ "entry": "fs_a" }] }),
            "more than 24 parameters",
        ),
    ];
    for (json, problem) in cases {
        let d = Descriptor::parse(&json.to_string(), "").unwrap();
        assert!(d.problems().iter().any(|p| p.contains(problem)), "{problem}: {:?}", d.problems());
    }
}

#[test]
fn the_contract_compiles_and_names_its_helpers() {
    let d = Descriptor::packaged(
        "solo",
        &json!({ "id": "solo", "passes": [{ "entry": "fs_solo" }], "cost": {} }).to_string(),
        "@fragment\nfn fs_solo(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }",
    )
    .unwrap();
    assert!(d.uses.is_empty() && !d.newer());
    for helper in [
        "fn blend_mode(",
        "fn rotate2(",
        "fn greyscale(",
        "fn hsv(",
        "fn from_hsv(",
        "fn tex_size(",
        "fn daytime(",
        "fn band_left(",
        "fn band_right(",
        "fn band_of(",
        "fn band_left_of(",
        "fn band_right_of(",
        "fn layer_color(",
        "fn layer_alpha(",
        "fn parallax(",
        "const PI",
        "aux7",
    ] {
        assert!(CONTRACT.contains(helper), "the contract lacks {helper}");
    }
}
