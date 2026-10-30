use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Pixels, Renderer, SceneAssets, SceneView};
use wprs_scene::{Quality, Runtime, SCHEMA, Scene, Values};

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

fn layers(post: Value) -> Value {
    let spin = json!({ "timeline": "spin", "keys": [[0, 0], [4, 360]] });
    let slide = json!({ "timeline": "spin", "keys": [[0, [30, 60]], [4, [130, 60]]] });
    json!([
        { "id": "back", "type": "image", "image": "photo", "size": [160, 90], "effects": [{ "effect": "blur", "params": { "radius": 4 } }] },
        { "id": "star", "type": "shape", "shape": { "star": { "points": 5, "size": 24 } }, "fill": "#ffcc66",
          "transform": { "position": [40, 30], "rotation": spin } },
        { "id": "card", "type": "solid", "color": "#4060ff80", "size": [40, 30], "blend": "overlay", "transform": { "position": [45, 35] } },
        { "id": "dot", "type": "shape", "shape": { "ellipse": { "size": [10, 10] } }, "fill": "#ffffff", "transform": { "position": slide } },
        { "id": "post", "type": "adjustment", "effects": [post] }
    ])
}

fn setup(r: &Renderer, post: Value) -> (Runtime, SceneView, SceneAssets) {
    let timelines = [json!({ "id": "spin", "duration": 4, "loop": "repeat" })];
    let canvas = json!({ "size": [160, 90], "background": "#203040" });
    let assets = json!({ "photo": { "path": "photo.png" } });
    let text = json!({ "schema": SCHEMA, "canvas": canvas, "assets": assets, "timelines": timelines, "layers": layers(post) });
    let scene = Scene::load(&text.to_string(), &Default::default()).unwrap();
    let photo = RgbaImage::from_fn(64, 64, |x, y| Rgba([x as u8 * 4, y as u8 * 4, (x ^ y) as u8 * 4, 255]));
    let files =
        SceneAssets { images: [("photo".to_string(), Pixels::new(photo))].into(), ..Default::default() };
    let view = r.scene_view(&scene, &files, &Values::new()).unwrap();
    (Runtime::new(Arc::new(scene), Values::new()), view, files)
}

fn pixels(r: &Renderer, view: &mut SceneView, runtime: &Runtime) -> Vec<u8> {
    let frame = runtime.frame(view.view(SIZE), Quality::Q3);
    let image = r.render_scene(view, runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
    let layer = Layer {
        image: Some(image),
        fit: Fit::Stretch,
        background: [0; 3],
        opacity: 1.0,
        uv: None,
        relief: None,
    };
    let (mut target, mut out) = (r.create_target(SIZE.0, SIZE.1).unwrap(), Vec::new());
    r.render_layers(&mut target, &[layer], &mut out).unwrap();
    out
}

fn compare(post: Value) -> Vec<u32> {
    let Some(r) = renderer() else { return Vec::new() };
    let (mut runtime, mut view, files) = setup(r, post.clone());
    let mut damage = Vec::new();
    for step in 0..12 {
        runtime.advance(if step == 6 { 0.0 } else { 0.2 });
        let partial = pixels(r, &mut view, &runtime);
        damage.push(view.stats().damage);
        let mut fresh = r.scene_view(&runtime.scene, &files, &Values::new()).unwrap();
        let whole = pixels(r, &mut fresh, &runtime);
        let off = partial.iter().zip(&whole).filter(|(a, b)| a.abs_diff(**b) > 2).count();
        assert_eq!(off, 0, "frame {step} differs from a full redraw in {off} bytes ({post})");
    }
    damage
}

#[test]
fn partial_redraws_match_full_ones_and_cover_only_what_moved() {
    let damage = compare(json!({ "effect": "vignette", "params": { "intensity": 0.4 } }));
    let Some((first, rest)) = damage.split_first() else { return };
    assert_eq!(*first, 100);
    assert!(rest.iter().all(|d| *d < 50), "damage per frame {damage:?}");
    assert_eq!(damage[6], 0, "a frame where nothing moved redrew {damage:?}");
}

#[test]
fn effects_that_read_around_a_change_redraw_what_they_cover() {
    let damage = compare(json!({ "effect": "blur", "params": { "radius": 3 } }));
    assert!(damage.iter().enumerate().all(|(i, d)| *d == 100 || (i == 6 && *d == 0)), "{damage:?}");
}

#[test]
fn the_luminance_grid_follows_the_bright_parts_of_the_frame() {
    let Some(r) = renderer() else { return };
    let half = json!([{ "id": "white", "type": "solid", "color": "#ffffff", "size": [80, 90], "transform": { "position": [40, 45] } }]);
    let text =
        json!({ "schema": SCHEMA, "canvas": { "size": [160, 90], "background": "#000000" }, "layers": half });
    let scene = Scene::load(&text.to_string(), &Default::default()).unwrap();
    let mut view = r.scene_view(&scene, &SceneAssets::default(), &Values::new()).unwrap();
    let runtime = Runtime::new(Arc::new(scene), Values::new());
    let frame = runtime.frame(view.view(SIZE), Quality::Q3);
    let image = r.render_scene(&mut view, &runtime, &frame, SIZE, Quality::Q3, &[]).unwrap().clone();
    let (mut meter, mut luma) = (r.meter(), Vec::new());
    assert!(!r.luma(&mut meter, Some(&image), &mut luma).unwrap() && luma.is_empty());
    r.finish().unwrap();
    let read = (0..100).any(|_| {
        r.luma(&mut meter, None, &mut luma).unwrap() || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            false
        }
    });
    assert!(read, "the grid was not read once the frame finished");
    assert!(!r.luma(&mut meter, None, &mut luma).unwrap(), "a grid was read twice");
    let [w, h] = wprs_scene::FLASH_GRID.map(|v| v as usize);
    assert_eq!(luma.len(), w * h);
    let at = |x: usize, y: usize| luma[y * w + x];
    assert!(at(0, 0) > 0.97 && at(w / 2 - 1, h - 1) > 0.97, "{:?}", &luma[..w]);
    assert!(at(w / 2, 0) < 0.03 && at(w - 1, h - 1) < 0.03, "{:?}", &luma[..w]);
}

#[test]
fn property_and_script_colour_changes_redraw_the_layers_they_touch() {
    let Some(r) = renderer() else { return };
    let spin = json!({ "timeline": "t", "keys": [[0, 0], [4, 360]] });
    let dot = json!({ "id": "spin", "type": "shape", "shape": { "ellipse": { "size": [4, 4] } }, "fill": "#000000",
        "transform": { "position": [60, 45], "rotation": spin } });
    let lit = json!({ "id": "lit", "type": "solid", "color": "#808080", "size": [60, 90], "transform": { "position": [30, 45] },
        "brightness": { "prop": "b" } });
    let hue = json!({ "id": "hue", "type": "group", "layers": [{ "id": "fill", "type": "solid", "color": { "prop": "c" },
        "size": [60, 90], "transform": { "position": [90, 45] } }] });
    let timelines = [json!({ "id": "t", "duration": 4, "loop": "repeat" })];
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [160, 90], "background": "#000000" },
        "timelines": timelines, "layers": [dot, lit, hue] });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let values = |b: f32, c: &str| -> Values {
        [("b".into(), json!(b)), ("c".into(), json!(c))].into_iter().collect()
    };
    let first = values(1.0, "#ffffff");
    let mut view = r.scene_view(&scene, &SceneAssets::default(), &first).unwrap();
    let mut runtime = Runtime::new(Arc::new(scene), first);
    let paint = |(b, c): (f32, &str), runtime: &mut Runtime, view: &mut SceneView, script: bool| {
        runtime.set_props(values(b, c));
        let color = script.then_some([0.1, 0.9, 0.2, 1.0]);
        let set = wprs_scene::LayerOverride { color, ..Default::default() };
        runtime.set_script(wprs_scene::ScriptOutputs {
            layers: [("fill".into(), set)].into(),
            ..Default::default()
        });
        runtime.advance(0.1);
        let partial = pixels(r, view, runtime);
        let mut fresh = r.scene_view(&runtime.scene, &SceneAssets::default(), runtime.props()).unwrap();
        let off =
            partial.iter().zip(&pixels(r, &mut fresh, runtime)).filter(|(a, b)| a.abs_diff(**b) > 2).count();
        assert_eq!(off, 0, "brightness {b}, colour {c}, script {script}: {off} bytes are stale");
        view.stats().damage
    };
    paint((1.0, "#ffffff"), &mut runtime, &mut view, false);
    paint((1.0, "#ffffff"), &mut runtime, &mut view, false);
    for change in [((0.2, "#ffffff"), false), ((0.2, "#202020"), false), ((0.2, "#202020"), true)] {
        let damage = paint(change.0, &mut runtime, &mut view, change.1);
        assert!(damage > 30 && damage < 100, "{change:?} redrew {damage}");
    }
}
