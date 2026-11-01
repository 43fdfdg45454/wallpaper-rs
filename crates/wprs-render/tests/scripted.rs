use serde_json::{Value, json};
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Renderer, SceneAssets};
use wprs_scene::{
    EffectControl, Extent, LayerOverride, Param, Quality, Runtime, SCHEMA, Scene, ScriptOutputs, Values,
};

const SIZE: (u32, u32) = (160, 90);

fn renderer() -> Option<&'static Renderer> {
    match Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

fn scene(layers: Value, templates: Value) -> Scene {
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [160, 90], "background": "#102030" },
        "templates": templates, "layers": layers });
    Scene::load(&doc.to_string(), &Default::default()).unwrap()
}

fn pixels(r: &Renderer, scene: Scene, outputs: ScriptOutputs) -> Vec<u8> {
    let mut view = r.scene_view(&scene, &SceneAssets::default(), &Values::new()).unwrap();
    let mut runtime = Runtime::new(Arc::new(scene), Values::new());
    runtime.set_script(outputs);
    let frame = runtime.frame(view.view(SIZE), Quality::Q3);
    let image = r.render_scene(&mut view, &runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
    let layer = Layer {
        image: Some(image),
        fit: Fit::Stretch,
        background: [0; 3],
        opacity: 1.0,
        uv: None,
        relief: None,
        flip: [false; 2],
        grade: None,
    };
    let (mut target, mut out) = (r.create_target(SIZE.0, SIZE.1).unwrap(), Vec::new());
    r.render_layers(&mut target, &[layer], &mut out).unwrap();
    out
}

fn same(a: &[u8], b: &[u8], what: &str) {
    let off = a.iter().zip(b).filter(|(x, y)| x.abs_diff(**y) > 2).count();
    assert_eq!(off, 0, "{what}: {off} bytes differ");
}

#[test]
fn script_writes_draw_like_the_same_fields_in_the_scene() {
    let Some(r) = renderer() else { return };
    let plain = json!([
        { "id": "card", "type": "solid", "color": "#ff8040", "size": [40, 20], "transform": { "position": [60, 40] } },
        { "id": "word", "type": "text", "text": "Hi", "size": 14, "font": "sans-serif", "color": "#ffffff", "transform": { "position": [110, 60] } }
    ]);
    let fixed = json!([
        { "id": "card", "type": "solid", "color": "#ff8040", "size": [30, 10],
          "transform": { "position": [60, 40], "skew": [20, 0], "origin": [0, 1] } },
        { "id": "word", "type": "text", "text": "Ok", "size": 20, "font": "serif", "align": "left", "color": "#ffffff", "transform": { "position": [110, 60] } }
    ]);
    let card = LayerOverride {
        skew: Some([20.0, 0.0]),
        origin: Some([0.0, 1.0]),
        size: Some(Extent::Box([30.0, 10.0])),
        ..LayerOverride::default()
    };
    let word = LayerOverride {
        text: Some("Ok".into()),
        size: Some(Extent::Points(20.0)),
        font: Some("serif".into()),
        align: Some(wprs_scene::Align::Left),
        ..LayerOverride::default()
    };
    let outputs = ScriptOutputs {
        layers: [("card".into(), card), ("word".into(), word)].into(),
        ..ScriptOutputs::default()
    };
    let scripted = pixels(r, scene(plain.clone(), json!({})), outputs);
    same(&scripted, &pixels(r, scene(fixed, json!({})), ScriptOutputs::default()), "layer writes");
    assert_ne!(scripted, pixels(r, scene(plain, json!({})), ScriptOutputs::default()));
}

#[test]
fn script_effect_controls_draw_like_the_same_parameters() {
    let Some(r) = renderer() else { return };
    let layers = |radius: f32, effects: bool| {
        let mut back = json!({ "id": "back", "type": "shape", "shape": { "star": { "points": 5, "size": 30 } }, "fill": "#ffcc66",
            "transform": { "position": [80, 45] } });
        if effects {
            back["effects"] = json!([{ "effect": "blur", "name": "soft", "params": { "radius": radius } }, { "effect": "pixelate" }]);
        }
        json!([back])
    };
    let params = [("radius".to_string(), Param::Number(2.0))].into();
    let controls = vec![
        EffectControl { at: 0, params, ..EffectControl::default() },
        EffectControl { at: 1, enabled: Some(false), ..EffectControl::default() },
    ];
    let outputs = ScriptOutputs { effects: [("back".into(), controls)].into(), ..ScriptOutputs::default() };
    let scripted = pixels(r, scene(layers(8.0, true), json!({})), outputs);
    let mut fixed = layers(2.0, true);
    fixed[0]["effects"].as_array_mut().unwrap().pop();
    same(&scripted, &pixels(r, scene(fixed, json!({})), ScriptOutputs::default()), "effect controls");
    let off = EffectControl { at: 0, enabled: Some(false), ..EffectControl::default() };
    let outputs = ScriptOutputs {
        effects: [("back".into(), vec![off.clone(), EffectControl { at: 1, ..off }])].into(),
        ..ScriptOutputs::default()
    };
    let bare = pixels(r, scene(layers(8.0, false), json!({})), ScriptOutputs::default());
    same(&pixels(r, scene(layers(8.0, true), json!({})), outputs), &bare, "effects switched off");
}

#[test]
fn layers_made_from_templates_draw_like_written_layers_in_their_order() {
    let Some(r) = renderer() else { return };
    let bar = json!({ "id": "bar", "type": "group", "layers": [
        { "id": "fill", "type": "solid", "color": "#40c0ff", "size": [12, 50] },
        { "id": "cap", "type": "solid", "color": "#ffffff", "size": [12, 4], "transform": { "position": [0, -25] } }
    ], "effects": [{ "effect": "blur", "params": { "radius": 1 } }] });
    let floor = json!({ "id": "floor", "type": "solid", "color": "#202020", "size": [160, 30], "transform": { "position": [80, 60] } });
    let placed = |id: &str, x: f32| {
        let mut b = bar.clone();
        b["id"] = json!(id);
        b["transform"] = json!({ "position": [x, 50] });
        for child in b["layers"].as_array_mut().unwrap() {
            child["id"] = json!(format!("{id}_{}", child["id"].as_str().unwrap()));
        }
        b
    };
    let fixed = scene(json!([placed("b1", 40.0), floor.clone(), placed("b2", 90.0)]), json!({}));
    let spawned = vec![("b1".to_string(), "bar".to_string()), ("b2".into(), "bar".into())];
    let at = |x: f32| LayerOverride { position: Some([x, 50.0]), ..LayerOverride::default() };
    let outputs = ScriptOutputs {
        spawned,
        order: Some(vec!["b1".into(), "floor".into(), "b2".into()]),
        layers: [("b1".into(), at(40.0)), ("b2".into(), at(90.0))].into(),
        ..ScriptOutputs::default()
    };
    let scripted = pixels(r, scene(json!([floor]), json!({ "bar": bar })), outputs);
    same(&scripted, &pixels(r, fixed, ScriptOutputs::default()), "template layers");
}
