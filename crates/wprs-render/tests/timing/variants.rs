use super::hostile::renderer;
use super::one;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_render::SceneAssets;
use wprs_scene::{Property, Quality, Runtime, SCHEMA, Scene, Values};

#[test]
fn option_values_outside_the_built_variants_never_build_on_the_render_thread() {
    let _one = one();
    let Some(r) = renderer() else { return };
    let json = json!({ "id": "vv", "cost": {}, "options": [{ "id": "o", "values": [1, 0], "default": 1 }],
        "passes": [{ "entry": "fs_v" }] });
    let wgsl = "override o: u32 = 1u; override z: u32 = 4u / o;
        @fragment fn fs_v(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * f32(z) * 0.25; }";
    let used = json!({ "effect": "vv", "params": { "o": { "prop": "p", "map": { "a": 1 } } } });
    let layers = json!([{ "id": "bg", "type": "solid", "color": "#ff0000", "effects": [used] }]);
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "layers": layers,
        "effects": { "vv": { "descriptor": "fx/vv.json", "wgsl": "fx/vv.wgsl" } } });
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let props: BTreeMap<String, Property> = serde_json::from_value(json!({
        "p": { "type": "slider", "min": 0, "max": 1, "step": 1, "integer": true, "default": 0 } }))
    .unwrap();
    let files = |path: &str, _: usize| {
        Ok::<_, &str>(if path.ends_with(".json") { json.to_string() } else { wgsl.to_string() }.into_bytes())
    };
    scene.adopt(&props, files).unwrap();
    let values: Values = [("p".to_string(), json!(0))].into_iter().collect();
    let mut view = r.scene_view(&scene, &SceneAssets::default(), &values).unwrap();
    let runtime = Runtime::new(Arc::new(scene), values);
    for _ in 0..2 {
        let frame = runtime.frame(view.view((64, 32)), Quality::Q3);
        r.render_scene(&mut view, &runtime, &frame, (64, 32), Quality::Q3, &[]).unwrap();
        assert!(view.left_out().is_empty(), "{:?}", view.left_out());
    }
    r.finish().unwrap();
    assert!(!r.lost());
}
