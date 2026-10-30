use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::*;

const WGSL: &str = r#"
override taps: i32 = 4;
override tinted: bool = false;

@fragment
fn fs_ripple_step(in: VertexOut) -> @location(0) vec4<f32> {
    let wave = sin(frame.time * 3.0 + in.uv.x * f32(taps));
    return tap(in.uv) * wave + sample(aux4, in.uv) * band_left(2u) + sample(aux7, in.uv) * 0.0;
}

@vertex
fn vs_ripple(@builtin(vertex_index) i: u32) -> VertexOut {
    let p = vec2<f32>(f32(i % 2u), f32(i / 2u));
    return VertexOut(vec4<f32>(rotate2(p * 2.0 - 1.0, 5.0), 0.0, 1.0), p);
}

@fragment
fn fs_ripple(in: VertexOut) -> @location(0) vec4<f32> {
    let c = tap(in.uv);
    let mixed = blend_mode(u32(param(2u).x), c.rgb, from_hsv(hsv(c.rgb)), 0.5) * greyscale(c.rgb);
    let tinted_rgb = select(mixed, mixed * layer_color().rgb, tinted);
    return vec4<f32>(tinted_rgb + vec3<f32>(tex_size(3u), daytime()) * 0.0 + band_of(1u, 16u) * 0.0, c.a * layer_alpha());
}
"#;

fn descriptor() -> Json {
    json!({ "id": "ripple", "name": { "en": "Ripple", "es": "Onda" },
        "params": [
            { "id": "detail", "type": "int", "default": 1, "min": 0, "max": 4, "label": { "en": "Detail", "es": "Detalle" } },
            { "id": "style", "type": "enum", "default": 0, "values": [{ "value": 0, "label": { "en": "Soft" } }, { "value": 1 }] },
            { "id": "mode", "type": "blend", "default": "screen", "help": { "en": "How the waves mix" } },
            { "id": "center", "type": "position", "default": [0.5, 0.5] },
            { "id": "source", "type": "texture", "default": "" },
            { "id": "paint", "type": "mask", "default": "", "channels": 3, "paint": "#000000" }
        ],
        "targets": { "history": { "scale": 0.5, "format": "rgba16f" } },
        "options": [{ "id": "taps", "values": [4, 8], "default": 4 },
                    { "id": "tinted", "values": [false, true], "default": false, "when_bound": "paint" }],
        "passes": [
            { "entry": "fs_ripple_step", "scale": 0.5, "target": "history", "blend": "add",
              "inputs": ["previous", "target:history", "backdrop", "asset:source", "asset:paint"] },
            { "copy": "target:history" },
            { "entry": "fs_ripple", "vertex": "vs_ripple", "grid": [16, 16], "inputs": ["input", "pass:1"] }
        ],
        "cost": { "taps": 4, "passes": 3, "scale": 1 } })
}

fn packaged(change: impl FnOnce(&mut Json), wgsl: &str) -> Result<Descriptor, String> {
    let mut d = descriptor();
    change(&mut d);
    Descriptor::packaged("ripple", &d.to_string(), wgsl)
}

#[test]
fn package_effects_compile_against_the_contract_and_report_what_they_read() {
    let d = packaged(|_| (), WGSL).unwrap_or_else(|e| panic!("{e}"));
    assert!(d.uses.contains(&Use::Time) && d.uses.contains(&Use::Audio) && !d.uses.contains(&Use::Cursor));
    let mut first =
        builtin().iter().filter(|b| !["tint", "blend_texture", "light_shafts"].contains(&b.id.as_str()));
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
        (params(json!({ "id": "k", "type": "blend", "default": "tint" })), "default of k does not fit"),
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

fn scene(uses: Json) -> Scene {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "assets": { "a": { "path": "a.png" } },
        "effects": { "ripple": { "descriptor": "fx/ripple.json", "wgsl": "fx/ripple.wgsl" } },
        "layers": [{ "id": "l", "type": "image", "image": "a", "effects": uses }] });
    Scene::load(&text.to_string(), &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"))
}

fn adopt(scene: &mut Scene, descriptor: &Json) -> Result<(), SceneError> {
    let files = |path: &str, _: usize| match path {
        "fx/ripple.json" => Ok(descriptor.to_string().into_bytes()),
        "fx/ripple.wgsl" => Ok(WGSL.as_bytes().to_vec()),
        _ => Err("missing"),
    };
    scene.adopt(&BTreeMap::new(), files)
}

#[test]
fn scenes_adopt_their_package_effects_and_check_their_uses() {
    let mut fine =
        scene(json!([{ "effect": "ripple", "params": { "detail": 3, "taps": 8, "mode": "glow" } }]));
    assert_eq!(fine.features().into_iter().collect::<Vec<_>>(), ["custom_effects"]);
    adopt(&mut fine, &descriptor()).unwrap();
    let found: Vec<_> = fine.features().into_iter().collect();
    assert_eq!(found, ["custom_effects", "effect_options", "vertex_effects"]);
    assert!(fine.effect("ripple").is_some() && fine.effect("blur").is_some() && needs(&fine).audio);
    for (params, problem) in [
        (json!({ "detail": 9 }), "/layers/0/effects/0/params/detail is 9, expected 0..=4"),
        (json!({ "taps": 5 }), "/layers/0/effects/0/params/taps does not fit"),
        (json!({ "mode": "tint" }), "/layers/0/effects/0/params/mode does not fit"),
        (json!({ "missing": 1 }), "names the unknown parameter"),
    ] {
        let mut wrong = scene(json!([{ "effect": "ripple", "params": params }]));
        let fault = adopt(&mut wrong, &descriptor()).unwrap_err().to_string();
        assert!(fault.contains(problem), "{fault}");
    }
    let mut wide = descriptor();
    wide["options"] = json!([{ "id": "taps", "values": [1, 2, 3, 4, 5], "default": 1 },
        { "id": "tinted", "values": [false, true], "default": false }]);
    let props: BTreeMap<String, Property> = serde_json::from_value(json!({
        "q": { "type": "slider", "min": 1, "max": 8, "default": 1 }, "t": { "type": "bool", "default": false } })).unwrap();
    let mut bound = scene(
        json!([{ "effect": "ripple", "params": { "taps": { "prop": "q" }, "tinted": { "prop": "t" } } }]),
    );
    let files = |path: &str, _: usize| {
        Ok::<_, &str>(if path.ends_with("json") { wide.to_string() } else { WGSL.into() }.into_bytes())
    };
    let fault = bound.adopt(&props, files).unwrap_err().to_string();
    assert_eq!(
        fault,
        "invalid scene: /layers/0/effects/0/params needs 10 variants of its shader, at most 8 are allowed"
    );
    let mut waiting = descriptor();
    waiting["options"][0]["values"] = json!([1, 2, 3, 4, 5]);
    waiting["options"][0]["default"] = json!(1);
    let mut both = scene(json!([{ "effect": "ripple",
        "params": { "taps": { "prop": "q" }, "paint": { "prop": "t", "map": { "false": "a", "true": "a" } } } }]));
    let files = |path: &str, _: usize| {
        Ok::<_, &str>(if path.ends_with("json") { waiting.to_string() } else { WGSL.into() }.into_bytes())
    };
    let fault = both.adopt(&props, files).unwrap_err().to_string();
    assert!(fault.ends_with("needs 10 variants of its shader, at most 8 are allowed"), "{fault}");
    let d = packaged(|_| (), WGSL).unwrap();
    let fed: EffectUse =
        serde_json::from_value(json!({ "effect": "ripple", "params": { "paint": { "layer": "sky" } } }))
            .unwrap();
    assert_eq!((d.variant(&fed, |_| None), d.variants(&fed).unwrap()), (1 << 4, vec![1 << 4]));
    let painted: EffectUse =
        serde_json::from_value(json!({ "effect": "ripple", "params": { "paint": "a" } })).unwrap();
    assert_eq!(d.variants(&painted).unwrap(), vec![1 << 4]);
    let plain: EffectUse = serde_json::from_value(json!({ "effect": "ripple" })).unwrap();
    assert_eq!((d.variant(&plain, |_| None), d.variants(&plain).unwrap()), (0, vec![0]));
    let mut gone = scene(json!([{ "effect": "ripple" }]));
    let files = |_: &str, _: usize| Err::<Vec<u8>, _>("no such file");
    assert!(
        gone.adopt(&BTreeMap::new(), files)
            .unwrap_err()
            .to_string()
            .contains("/effects/ripple: fx/ripple.json")
    );
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

#[test]
fn package_effects_compile_once_and_share_a_shader_budget() {
    let (mut first, mut second) =
        (scene(json!([{ "effect": "ripple" }])), scene(json!([{ "effect": "ripple" }])));
    adopt(&mut first, &descriptor()).unwrap();
    adopt(&mut second, &descriptor()).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first.loaded["ripple"], &second.loaded["ripple"]));
    let effects: serde_json::Map<String, Json> = (0..5)
        .map(|i| {
            (
                format!("e{i}"),
                json!({ "descriptor": format!("fx/e{i}.json"), "wgsl": format!("fx/e{i}.wgsl") }),
            )
        })
        .collect();
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "effects": effects, "layers": [] });
    let mut many = Scene::load(&text.to_string(), &BTreeMap::new()).unwrap();
    let files = |path: &str, max: usize| {
        let id = path.trim_start_matches("fx/").split('.').next().unwrap_or_default();
        let mut d = descriptor();
        d["id"] = json!(id);
        let text =
            if path.ends_with("json") { d.to_string() } else { format!("{WGSL}{}", " ".repeat(60 << 10)) };
        Some(text.into_bytes()).filter(|b| b.len() <= max).ok_or("too large")
    };
    let fault = many.adopt(&BTreeMap::new(), files).unwrap_err().to_string();
    assert!(fault.contains("the shaders of the package pass 256 KiB"), "{fault}");
}
