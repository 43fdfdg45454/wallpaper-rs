use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Renderer, SceneAssets, SceneView};
use wprs_scene::{AudioFrame, Property, Quality, Runtime, SCHEMA, Scene, effective};

const SIZE: (u32, u32) = (100, 50);

fn renderer() -> Option<Renderer> {
    match Renderer::new() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

fn halves(left: [u8; 4], right: [u8; 4]) -> Arc<RgbaImage> {
    Arc::new(RgbaImage::from_fn(100, 50, |x, _| Rgba(if x < 50 { left } else { right })))
}

struct Shot {
    runtime: Runtime,
    view: SceneView,
}

impl Shot {
    fn new(r: &Renderer, canvas: Value, layers: Value, images: &[(&str, Arc<RgbaImage>)]) -> Shot {
        let mut text = json!({ "schema": SCHEMA, "canvas": canvas, "layers": layers });
        text["assets"] =
            images.iter().map(|(id, _)| (id.to_string(), json!({ "path": format!("{id}.png") }))).collect();
        let props: BTreeMap<String, Property> =
            serde_json::from_value(json!({ "tone": { "type": "color", "default": "#00ff00" } })).unwrap();
        let scene = Scene::load(&text.to_string(), &props).unwrap();
        let assets = SceneAssets {
            images: images.iter().map(|(id, i)| (id.to_string(), i.clone())).collect(),
            ..Default::default()
        };
        let view = r.scene_view(&scene, &assets).unwrap();
        Shot { runtime: Runtime::new(Arc::new(scene), effective(&props, &[])), view }
    }

    fn pixels(&mut self, r: &Renderer) -> Vec<u8> {
        let frame = self.runtime.frame(self.view.view(SIZE), Quality::Q3);
        let image = r.render_scene(&mut self.view, &self.runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
        let mut target = r.create_target(SIZE.0, SIZE.1).unwrap();
        let mut pixels = Vec::new();
        let layer = Layer {
            image: Some(image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv: None,
            relief: None,
        };
        r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
        pixels
    }
}

fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [pixels[i + 2], pixels[i + 1], pixels[i]]
}

fn near(found: [u8; 3], expected: [u8; 3], tolerance: u8) -> bool {
    found.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= tolerance)
}

fn check(pixels: &[u8], points: &[((u32, u32), [u8; 3])]) {
    for &((x, y), expected) in points {
        assert!(
            near(at(pixels, x, y), expected, 3),
            "pixel {x},{y} is {:?}, expected {expected:?}",
            at(pixels, x, y)
        );
    }
}

fn canvas(background: &str) -> Value {
    json!({ "size": [100, 50], "background": background })
}

#[test]
fn layers_compose_in_place() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "img", "type": "image", "image": "rb", "size": [100, 50] },
        { "id": "box", "type": "solid", "color": { "prop": "tone" }, "size": [20, 10], "transform": { "position": [10, 5], "origin": [0, 0] } },
        { "id": "dot", "type": "shape", "shape": { "ellipse": { "size": [10, 10] } }, "fill": "#ffffff", "anchor": "bottom_right", "transform": { "origin": [0.5, 0.5], "position": [-10, -10] } },
        { "id": "half", "type": "solid", "color": "#ffffff", "opacity": 0.5, "size": [10, 10], "transform": { "position": [80, 10] } }
    ]);
    let mut shot =
        Shot::new(&r, canvas("#000000"), layers, &[("rb", halves([255, 0, 0, 255], [0, 0, 255, 255]))]);
    let pixels = shot.pixels(&r);
    check(
        &pixels,
        &[
            ((2, 40), [255, 0, 0]),
            ((97, 40), [0, 0, 255]),
            ((15, 8), [0, 255, 0]),
            ((90, 40), [255, 255, 255]),
        ],
    );
    check(&pixels, &[((96, 46), [0, 0, 255]), ((80, 10), [188, 188, 255])]);
}

#[test]
fn blend_modes_and_blend_spaces() {
    let Some(r) = renderer() else { return };
    let modes = [
        ("normal", [255, 0, 0]),
        ("multiply", [128, 0, 0]),
        ("screen", [255, 128, 128]),
        ("add", [255, 128, 128]),
        ("difference", [229, 128, 128]),
        ("darken", [128, 0, 0]),
        ("luminosity", [149, 149, 149]),
    ];
    for (mode, expected) in modes {
        let layers = json!([{ "id": "s", "type": "solid", "color": "#ff0000", "blend": mode, "size": [50, 50], "transform": { "position": [75, 25] } }]);
        let pixels = Shot::new(&r, canvas("#808080"), layers, &[]).pixels(&r);
        assert!(near(at(&pixels, 75, 25), expected, 4), "{mode}: {:?}", at(&pixels, 75, 25));
        assert!(near(at(&pixels, 25, 25), [128, 128, 128], 2), "{mode} backdrop");
    }
    let fade = |space: &str| {
        let layers = json!([{ "id": "s", "type": "solid", "color": "#ffffff", "opacity": 0.5 }]);
        let canvas = json!({ "size": [100, 50], "background": "#000000", "blend_space": space });
        at(&Shot::new(&r, canvas, layers, &[]).pixels(&r), 50, 25)
    };
    assert!(near(fade("linear"), [188, 188, 188], 2) && near(fade("srgb"), [128, 128, 128], 2));
}

#[test]
fn masks_cut_layers() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "tex", "type": "solid", "color": "#ff0000", "size": [50, 50], "transform": { "position": [25, 25] }, "mask": "lr" },
        { "id": "round", "type": "solid", "color": "#00ff00", "size": [50, 50], "transform": { "position": [75, 25] }, "mask": { "shape": { "ellipse": { "size": [30, 30] } } } }
    ]);
    let pixels =
        Shot::new(&r, canvas("#000000"), layers, &[("lr", halves([255; 4], [0, 0, 0, 255]))]).pixels(&r);
    check(
        &pixels,
        &[((10, 25), [255, 0, 0]), ((40, 25), [0, 0, 0]), ((75, 25), [0, 255, 0]), ((53, 3), [0, 0, 0])],
    );
    for (visible, matte) in [(true, [0, 255, 0]), (false, [0, 0, 0])] {
        let layers = json!([
            { "id": "round", "type": "shape", "shape": { "ellipse": { "size": [30, 30] } }, "fill": "#00ff00",
              "transform": { "position": [75, 25] }, "visible": visible },
            { "id": "inverse", "type": "solid", "color": "#0000ff", "mask": { "layer": "round", "invert": true } }
        ]);
        let pixels = Shot::new(&r, canvas("#000000"), layers, &[]).pixels(&r);
        check(&pixels, &[((75, 25), matte), ((10, 25), [0, 0, 255]), ((53, 3), [0, 0, 255])]);
    }
}

#[test]
fn effects_render_their_passes() {
    let Some(r) = renderer() else { return };
    let rb = || halves([255, 0, 0, 255], [0, 0, 255, 255]);
    let gray = json!([{ "id": "s", "type": "solid", "color": "#ff0000", "effects": [{ "effect": "color_adjust", "params": { "saturation": 0 } }] }]);
    check(&Shot::new(&r, canvas("#000000"), gray, &[]).pixels(&r), &[((50, 25), [127, 127, 127])]);
    let blur = json!([{ "id": "i", "type": "image", "image": "rb", "size": [100, 50], "effects": [{ "effect": "blur", "params": { "radius": 12 } }] }]);
    let pixels = Shot::new(&r, canvas("#000000"), blur, &[("rb", rb())]).pixels(&r);
    let [red, _, blue] = at(&pixels, 50, 25);
    assert!(red > 60 && blue > 60 && red < 250 && blue < 250, "blurred edge {:?}", at(&pixels, 50, 25));
    check(&pixels, &[((25, 25), [255, 0, 0]), ((75, 25), [0, 0, 255])]);
    let waves = json!([{ "id": "i", "type": "image", "image": "rb", "size": [100, 50],
        "effects": [{ "effect": "waves", "params": { "amplitude": 10, "frequency": 1, "speed": 0, "direction": 90 } }] }]);
    let pixels = Shot::new(&r, canvas("#000000"), waves, &[("rb", rb())]).pixels(&r);
    check(&pixels, &[((55, 12), [255, 0, 0]), ((45, 37), [0, 0, 255]), ((25, 25), [255, 0, 0])]);
    let glow = json!([{ "id": "s", "type": "solid", "color": "#ffffff", "size": [10, 10],
        "effects": [{ "effect": "bloom", "params": { "threshold": 0.5, "intensity": 2, "radius": 8 } }] }]);
    let pixels = Shot::new(&r, canvas("#000000"), glow, &[]).pixels(&r);
    assert!(at(&pixels, 58, 25)[0] > 20 && at(&pixels, 50, 25) == [255; 3], "{:?}", at(&pixels, 58, 25));
    check(&pixels, &[((5, 5), [0, 0, 0])]);
}

#[test]
fn groups_isolate_and_spectra_follow_audio() {
    let Some(r) = renderer() else { return };
    let layers = json!([{ "id": "g", "type": "group", "opacity": 0.5, "layers": [
        { "id": "a", "type": "solid", "color": "#ff0000", "size": [60, 50], "transform": { "position": [30, 25] } },
        { "id": "b", "type": "solid", "color": "#0000ff", "size": [60, 50], "transform": { "position": [70, 25] } }
    ] }]);
    check(
        &Shot::new(&r, canvas("#000000"), layers, &[]).pixels(&r),
        &[((10, 25), [188, 0, 0]), ((50, 25), [0, 0, 188]), ((90, 25), [0, 0, 188])],
    );
    let bars = json!([{ "id": "s", "type": "spectrum", "size": [100, 50], "bands": 16, "gap": 0.0, "color": "#ffffff" }]);
    let mut shot = Shot::new(&r, canvas("#000000"), bars, &[]);
    check(&shot.pixels(&r), &[((3, 25), [0, 0, 0])]);
    let mut audio = AudioFrame { silent: false, ..AudioFrame::default() };
    audio.average[..4].fill(1.0);
    audio.average[4..8].fill(0.5);
    assert!(shot.runtime.set_audio(audio));
    check(
        &shot.pixels(&r),
        &[
            ((3, 25), [255, 255, 255]),
            ((9, 12), [0, 0, 0]),
            ((9, 40), [255, 255, 255]),
            ((60, 40), [0, 0, 0]),
        ],
    );
}

#[test]
fn static_parts_are_cached_and_steady_frames_create_nothing() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "img", "type": "image", "image": "rb", "size": [100, 50], "effects": [{ "effect": "blur", "params": { "radius": 4 } }] },
        { "id": "spin", "type": "solid", "color": "#ffffff", "size": [10, 10], "transform": { "rotation": { "keys": [[0, 0], [10, 360]], "loop": "repeat" } } },
        { "id": "text", "type": "text", "text": "Hi", "size": 30, "color": "#ffffff", "transform": { "position": [20, 25] } },
        { "id": "pulse", "type": "solid", "color": "#00ff00", "size": [20, 20], "effects": [{ "effect": "blur", "params": { "radius": 2 } }],
          "transform": { "position": [80, 25], "scale": { "keys": [[0, 1], [10, 1.4]], "loop": "pingpong" } } }
    ]);
    let mut shot =
        Shot::new(&r, canvas("#000000"), layers, &[("rb", halves([255, 0, 0, 255], [0, 0, 255, 255]))]);
    let first = shot.pixels(&r);
    assert!(shot.view.stats().created > 0);
    shot.runtime.seek(1.25);
    let second = shot.pixels(&r);
    let stats = shot.view.stats();
    assert_eq!((stats.created, stats.reused), (0, 1), "{stats:?}");
    assert!(stats.passes >= 1 && stats.draws >= 2, "{stats:?}");
    assert_ne!(first, second);
    let text = (0..50)
        .flat_map(|x| (10..40).map(move |y| (x, y)))
        .filter(|&(x, y)| at(&second, x, y)[1] > 200)
        .count();
    let fonts = fontdb_has_fonts();
    assert!(!fonts || text > 10, "text pixels {text}");
    for t in [2.5, 5.0, 7.5] {
        shot.runtime.seek(t);
        shot.pixels(&r);
        assert_eq!(shot.view.stats().created, 0, "growing layer at {t} s");
    }
}

fn fontdb_has_fonts() -> bool {
    ["/usr/share/fonts", "/usr/local/share/fonts"].iter().any(|d| std::path::Path::new(d).is_dir())
}
