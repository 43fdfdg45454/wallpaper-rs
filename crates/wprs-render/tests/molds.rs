use serde_json::{Value, json};
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Renderer, SceneAssets};
use wprs_scene::{
    CameraSet, LayerOverride, Mesh, Quality, Runtime, SCHEMA, Scene, ScriptOutputs, StageSet, Values,
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

fn scene(extra: Value) -> Scene {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [160, 90], "background": "#102030" } });
    doc.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
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

fn rgb(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [px[i + 2], px[i + 1], px[i]]
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 3)
}

fn quad(x0: f32, y0: f32, x1: f32, y1: f32, c: [f32; 4]) -> (Vec<f32>, Vec<u32>) {
    let v = |x, y| [x, y, 0.0, 0.0, c[0], c[1], c[2], c[3]];
    ([v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)].concat(), vec![0, 1, 2, 0, 2, 3])
}

#[test]
fn script_shapes_draw_through_the_layer_that_shows_their_pool() {
    let Some(r) = renderer() else { return };
    let pooled = || {
        scene(json!({ "pools": { "bars": { "type": "mesh", "max": 4, "max_vertices": 64 } },
        "layers": [{ "id": "bars", "type": "video", "video": "bars" }] }))
    };
    let (mut vertices, mut indices) = quad(0.0, 0.0, 80.0, 90.0, [1.0, 0.0, 0.0, 1.0]);
    let (more, mut tail) = quad(100.0, 0.0, 160.0, 45.0, [0.0, 1.0, 0.0, 0.5]);
    tail.iter_mut().for_each(|i| *i += 4);
    vertices.extend(more);
    indices.extend(tail);
    let mesh = Mesh { generation: 1, shapes: 2, vertices, indices };
    let outputs =
        ScriptOutputs { meshes: [("bars".into(), Arc::new(mesh))].into(), ..ScriptOutputs::default() };
    let drawn = pixels(r, pooled(), outputs);
    assert!(near(rgb(&drawn, 20, 60), [255, 0, 0]), "{:?}", rgb(&drawn, 20, 60));
    assert!(near(rgb(&drawn, 90, 60), [16, 32, 48]), "{:?}", rgb(&drawn, 90, 60));
    let half = rgb(&drawn, 130, 20);
    assert!(half[1] > 150 && half[1] < 200 && half[0] < 40, "{half:?}");
    let empty = pixels(r, pooled(), ScriptOutputs::default());
    assert!(near(rgb(&empty, 20, 60), [16, 32, 48]), "an empty pool shows nothing");
    let wrong = Mesh { generation: 2, shapes: 1, vertices: vec![0.0; 8], indices: vec![0, 0, 5] };
    let refused =
        ScriptOutputs { meshes: [("bars".into(), Arc::new(wrong))].into(), ..ScriptOutputs::default() };
    assert!(near(rgb(&pixels(r, pooled(), refused), 0, 0), [16, 32, 48]), "indices out of range are refused");
}

#[test]
fn script_camera_and_parents_draw_like_the_same_scene_fields() {
    let Some(r) = renderer() else { return };
    let solid = |id: &str, at: [f32; 2], parent: Option<&str>| {
        let mut l = json!({ "id": id, "type": "solid", "color": "#ffaa00", "size": [20, 10], "transform": { "position": at } });
        parent.iter().for_each(|p| l["parent"] = json!(p));
        l
    };
    let plain =
        json!({ "layers": [solid("sun", [60.0, 40.0], None), solid("moon", [10.0, 5.0], Some("sun"))] });
    let alone = json!({ "layers": [solid("sun", [60.0, 40.0], None), solid("moon", [10.0, 5.0], None)] });
    let set = LayerOverride { parent: Some("sun".into()), ..LayerOverride::default() };
    let parented = ScriptOutputs { layers: [("moon".into(), set)].into(), ..ScriptOutputs::default() };
    let want = pixels(r, scene(plain.clone()), ScriptOutputs::default());
    assert_eq!(
        pixels(r, scene(alone), parented),
        want,
        "a parent set by a script moves like one in scene.json"
    );
    let mut zoomed = plain.clone();
    zoomed["canvas"] =
        json!({ "size": [160, 90], "background": "#304050", "camera": { "zoom": 2, "rotation": 10 } });
    let camera = CameraSet { zoom: Some(2.0), rotation: Some(10.0), ..CameraSet::default() };
    let back = [0x30 as f32 / 255.0, 0x40 as f32 / 255.0, 0x50 as f32 / 255.0, 1.0];
    let stage = StageSet { camera: Some(camera), background: Some(back), ..StageSet::default() };
    let moved = ScriptOutputs { stage: Some(stage), ..ScriptOutputs::default() };
    let want = pixels(r, scene(zoomed), ScriptOutputs::default());
    let got = pixels(r, scene(plain), moved);
    assert!(
        want.iter().zip(&got).all(|(a, b)| a.abs_diff(*b) <= 2),
        "a camera set by a script turns like one in scene.json"
    );
}
