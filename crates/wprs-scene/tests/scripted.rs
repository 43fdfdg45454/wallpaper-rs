use serde_json::json;
use std::sync::Arc;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 100.0, 50.0];

fn runtime(extra: serde_json::Value) -> Runtime {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] },
        "templates": { "bar": { "id": "bar", "type": "group", "layers": [{ "id": "fill", "type": "solid", "color": "#ffffff", "size": [4, 10] }] } },
        "layers": [
            { "id": "back", "type": "solid", "color": "#000000", "effects": [{ "effect": "blur", "name": "soft" }, { "effect": "water_ripple" }] },
            { "id": "label", "type": "text", "text": "a", "size": 12, "font": "serif" }
        ] });
    for (k, v) in extra.as_object().into_iter().flatten() {
        doc[k] = v.clone();
    }
    Runtime::new(Arc::new(Scene::from_json(&doc.to_string()).unwrap()), Values::new())
}

fn ids(rt: &Runtime) -> Vec<String> {
    rt.frame(VIEW, Quality::Q3).draws.iter().map(|d| d.layer.id.clone()).collect()
}

#[test]
fn templates_become_layers_in_the_order_scripts_give() {
    let mut rt = runtime(json!({}));
    let spawned = vec![
        ("one".to_string(), "bar".to_string()),
        ("two".into(), "bar".into()),
        ("back".into(), "bar".into()),
    ];
    assert!(rt.set_script(ScriptOutputs { spawned: spawned.clone(), ..ScriptOutputs::default() }));
    assert_eq!(ids(&rt), ["back", "label", "one", "one.fill", "two", "two.fill"]);
    let first = rt.spawned().next().map(|l| l as *const Layer);
    let order = Some(vec!["two".to_string(), "back".into(), "one".into()]);
    let spawned = vec![
        ("two".to_string(), "bar".to_string()),
        ("one".into(), "bar".into()),
        ("x".into(), "none".into()),
    ];
    assert!(rt.set_script(ScriptOutputs {
        spawned: spawned.clone(),
        order: order.clone(),
        ..ScriptOutputs::default()
    }));
    assert_eq!(ids(&rt), ["two", "two.fill", "back", "one", "one.fill", "label"]);
    assert_eq!(rt.spawned().nth(1).map(|l| l as *const Layer), first, "a kept layer moved in memory");
    assert!(!rt.set_script(ScriptOutputs {
        spawned,
        order,
        console: vec!["log".into()],
        ..ScriptOutputs::default()
    }));
    let many = (0..300).map(|i| (format!("b{i}"), "bar".to_string())).collect();
    rt.set_script(ScriptOutputs { spawned: many, ..ScriptOutputs::default() });
    assert_eq!(rt.spawned().count(), MAX_SPAWNED);
    assert!(rt.layer("b255.fill").is_some() && rt.layer("b256").is_none());
}

#[test]
fn effect_controls_keep_to_what_the_effect_declares() {
    let mut rt = runtime(json!({}));
    let mut params = std::collections::BTreeMap::new();
    params.insert("radius".to_string(), Param::Number(500.0));
    params.insert("glow".to_string(), Param::Number(1.0));
    params.insert("texture".to_string(), Param::Text("x".into()));
    let soft = EffectControl { at: 0, enabled: None, params, reset: 2 };
    let off = EffectControl { at: 1, enabled: Some(false), ..EffectControl::default() };
    let stray = EffectControl { at: 9, enabled: Some(false), ..EffectControl::default() };
    let effects =
        [("back".to_string(), vec![soft, off, stray]), ("label".into(), vec![EffectControl::default()])]
            .into();
    rt.set_script(ScriptOutputs { effects, ..ScriptOutputs::default() });
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(frame.effects.len(), 1);
    let blur = &frame.effects[0];
    assert_eq!((blur.descriptor.id.as_str(), blur.slots[0][0], blur.reset), ("blur", 128.0, 2));
}

#[test]
fn scripts_read_evaluated_layers_and_their_own_writes() {
    let mut rt = runtime(json!({}));
    let set = LayerOverride {
        rotation: Some(30.0),
        size: Some(Extent::Points(20.0)),
        font: Some("nowhere".into()),
        ..LayerOverride::default()
    };
    let mono = LayerOverride {
        font: Some("monospace".into()),
        text: Some("x".repeat(9000)),
        opacity: Some(4.0),
        ..LayerOverride::default()
    };
    rt.set_script(ScriptOutputs {
        layers: [("label".into(), set), ("back".into(), mono)].into(),
        ..ScriptOutputs::default()
    });
    let read = rt.read("label").unwrap();
    assert_eq!(
        (read.rotation, read.size, read.text.as_deref(), read.visible),
        (30.0, Some(Extent::Points(20.0)), Some("a"), true)
    );
    let frame = rt.frame(VIEW, Quality::Q3);
    let text = frame.draws.iter().find_map(|d| match &d.content {
        Content::Text { size, font, .. } => Some((*size, *font)),
        _ => None,
    });
    assert_eq!(text, Some((20.0, Some("serif"))));
    let back = rt.read("back").unwrap();
    assert_eq!((back.opacity, back.size, back.text), (1.0, None, None));
    assert!(rt.read("nothing").is_none());
}

#[test]
fn scripts_declare_their_inputs_by_what_they_import() {
    let source = r#"
        // import { audio } from "wprs";
        const text = "import { input } from 'wprs'";
        import { scene, particles as p, setTimeout } from "wprs";
        import * as math from "wprs/math";
    "#;
    assert_eq!(imported(source), [ScriptApi::Timers, ScriptApi::Particles, ScriptApi::Math]);
    assert_eq!(imported("import * as w from 'wprs'"), [ScriptApi::Audio, ScriptApi::Pointer]);
    assert_eq!(
        imported("import { audio as a, input } from \"wprs\""),
        [ScriptApi::Audio, ScriptApi::Pointer]
    );
    let scene = Scene::from_json(
        &json!({ "schema": SCHEMA, "canvas": { "size": [10, 10] },
        "scripts": [{ "path": "a.js" }, { "path": "b.js", "uses": ["timers"] }] })
        .to_string(),
    )
    .unwrap();
    let files = [
        ("a.js".to_string(), "/* audio cursor */ export const x = 1;".to_string()),
        ("b.js".into(), "import { audio, input } from 'wprs';".into()),
    ];
    assert_eq!(script_inputs(&scene, &files), [false, false]);
    let files = [("a.js".to_string(), "import { audio } from 'wprs';".to_string())];
    assert_eq!(script_inputs(&scene, &files), [true, false]);
}

#[test]
fn per_binding_values_are_found_by_their_site() {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [10, 10] },
        "scripts": [{ "path": "s.js", "params": { "k": { "type": "number", "default": 1 } } }],
        "templates": { "t": { "id": "t", "type": "solid", "color": "#ffffff", "opacity": { "script": "s.js", "fn": "fade" } } },
        "layers": [{ "id": "a", "type": "solid", "color": "#ffffff", "transform": { "rotation": { "script": "s.js", "fn": "turn", "params": { "k": 2 } } },
            "opacity": { "script": "s.js" } }] });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let found = sites(&scene);
    let keys: Vec<(&str, &str, Option<&str>)> =
        found.iter().map(|s| (s.path.as_str(), s.call.as_str(), s.layer.as_deref())).collect();
    assert_eq!(
        keys,
        [("/layers/0/transform/rotation", "turn", Some("a")), ("/templates/t/opacity", "fade", Some("t"))]
    );
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    let values = [(found[0].key.clone(), json!(45.0)), ("s.js".to_string(), json!(0.5))].into();
    rt.set_script(ScriptOutputs { values, ..ScriptOutputs::default() });
    let read = rt.read("a").unwrap();
    assert_eq!((read.rotation, read.opacity), (45.0, 0.5));
    for (bad, path) in [
        (json!({ "script": "s.js", "params": { "k": 2 } }), "/layers/0/opacity/fn"),
        (json!({ "script": "s.js", "fn": "update" }), "/layers/0/opacity/fn"),
    ] {
        let mut text = text.clone();
        text["layers"][0]["opacity"] = bad;
        let error = Scene::load(&text.to_string(), &Default::default()).unwrap_err().to_string();
        assert!(error.contains(path), "{error}");
    }
}

#[test]
fn templates_only_copy_what_scripts_can_draw() {
    let text = |kind: serde_json::Value| {
        json!({ "schema": SCHEMA, "canvas": { "size": [10, 10] }, "assets": { "v": { "path": "v.webm" } },
            "templates": { "t": kind } })
        .to_string()
    };
    let video = json!({ "id": "t", "type": "video", "video": "v" });
    assert!(
        Scene::load(&text(video), &Default::default()).unwrap_err().to_string().contains("/templates/t/type")
    );
    let shape =
        json!({ "id": "t", "type": "shape", "shape": { "rect": { "size": [2, 2] } }, "fill": "#ffffff" });
    assert!(Scene::load(&text(shape), &Default::default()).is_ok());
}
