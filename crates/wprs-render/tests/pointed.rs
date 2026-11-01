use serde_json::{Value, json};
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets, SceneView};
use wprs_scene::{Press, Quality, Runtime, SCHEMA, Scene, Values};

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

fn scene(layers: Value, timelines: Value) -> Scene {
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [160, 90], "background": "#102030" },
        "assets": { "half": { "path": "half.png" } }, "layers": layers, "timelines": timelines });
    Scene::load(&doc.to_string(), &Default::default()).unwrap()
}

fn assets() -> SceneAssets {
    let half =
        image::RgbaImage::from_fn(8, 8, |x, _| image::Rgba([255, 255, 255, if x < 4 { 0 } else { 255 }]));
    let mut assets = SceneAssets::default();
    assets.images.insert("half".into(), Pixels::new(half));
    assets
}

fn draw(r: &Renderer, view: &mut SceneView, runtime: &Runtime) -> Vec<u8> {
    let frame = runtime.frame(view.view(SIZE), Quality::Q3);
    let image = r.render_scene(view, runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
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
fn hitboxes_follow_transforms_groups_and_alpha() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "tilted", "type": "solid", "color": "#ff0000", "size": [40, 20], "solid": true, "transform": { "position": [40, 45], "rotation": 90 } },
        { "id": "pic", "type": "image", "image": "half", "size": [40, 40], "solid": { "alpha": 0.5 }, "transform": { "position": [120, 45] } },
        { "id": "deck", "type": "group", "solid": true, "transform": { "position": [80, 20] }, "layers": [
            { "id": "chip", "type": "solid", "color": "#00ff00", "size": [10, 10], "transform": { "position": [0, 0] } }] },
        { "id": "plain", "type": "solid", "color": "#0000ff", "size": [10, 10], "transform": { "position": [80, 80] } }]);
    let scene = scene(layers, json!([]));
    let mut view = r.scene_view(&scene, &assets(), &Values::new()).unwrap();
    let runtime = Runtime::new(Arc::new(scene), Values::new());
    draw(r, &mut view, &runtime);
    let hit = |x: f32, y: f32| view.hitboxes().hit([x, y]).map(|(l, at)| (l.to_owned(), at.map(f32::round)));
    assert_eq!(hit(40.0, 60.0).map(|h| h.0).as_deref(), Some("tilted"), "the rotation was not followed");
    assert_eq!(hit(55.0, 45.0), None, "the unrotated box took the pointer");
    assert_eq!(hit(110.0, 45.0), None, "a transparent part took the pointer");
    assert_eq!(hit(130.0, 45.0), Some(("pic".to_owned(), [30.0, 20.0])));
    assert_eq!(
        hit(80.0, 20.0),
        Some(("deck".to_owned(), [0.0, 0.0])),
        "groups take the pointer over their layers"
    );
    assert_eq!(hit(80.0, 80.0), None, "a layer that is not solid took the pointer");
}

#[test]
fn clicks_on_solid_layers_start_animations_and_drags_away_do_not() {
    let Some(r) = renderer() else { return };
    let lamp = |opacity: Value| json!({ "id": "lamp", "type": "solid", "color": "#ffffff", "size": [20, 20], "opacity": opacity, "transform": { "position": [140, 20] } });
    let button = json!({ "id": "button", "type": "solid", "color": "#2040c0", "size": [40, 20], "solid": true, "transform": { "position": [80, 45] } });
    let light = json!({ "timeline": "light", "keys": [[0, 0], [0.1, 1]] });
    let timelines = json!([{ "id": "light", "duration": 0.1, "autoplay": false, "trigger": "click:button" }]);
    let still = |opacity: f32| {
        let scene = scene(json!([button, lamp(json!(opacity))]), json!([]));
        let mut view = r.scene_view(&scene, &assets(), &Values::new()).unwrap();
        draw(r, &mut view, &Runtime::new(Arc::new(scene), Values::new()))
    };
    let (dark, lit) = (still(0.0), still(1.0));
    let pressed = |presses: &[Press]| {
        let scene = scene(json!([button, lamp(light.clone())]), timelines.clone());
        let mut view = r.scene_view(&scene, &assets(), &Values::new()).unwrap();
        let mut runtime = Runtime::new(Arc::new(scene), Values::new());
        draw(r, &mut view, &runtime);
        runtime.point(view.hitboxes(), view.view(SIZE), presses, (true, true));
        runtime.advance(0.2);
        draw(r, &mut view, &runtime)
    };
    let at = |x: f32, y: f32, buttons| Press { at: Some([x / 160.0, y / 90.0]), buttons };
    same(&pressed(&[at(80.0, 45.0, 1), at(82.0, 46.0, 0)]), &lit, "a click on the button");
    same(
        &pressed(&[at(80.0, 45.0, 1), at(150.0, 80.0, 1), at(150.0, 80.0, 0)]),
        &dark,
        "a drag off the button",
    );
    same(&pressed(&[at(10.0, 10.0, 1), at(80.0, 45.0, 0)]), &dark, "a release on the button");
}
