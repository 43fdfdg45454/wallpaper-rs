use image::{Rgba, RgbaImage};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use wprs_render::{Pixels, Renderer, SceneAssets, SceneView};
use wprs_scene::{Quality, Runtime, SCHEMA, Scene, Values};

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

fn scene(layers: serde_json::Value) -> Scene {
    let assets = json!({ "photo": { "path": "photo.png", "mipmaps": true } });
    let text =
        json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "assets": assets, "layers": layers });
    Scene::load(&text.to_string(), &Default::default()).unwrap()
}

fn draw(r: &Renderer, view: &mut SceneView, runtime: &Runtime) {
    let frame = runtime.frame(view.view((200, 100)), Quality::Q3);
    r.render_scene(view, runtime, &frame, (200, 100), Quality::Q3, &[]).unwrap();
}

#[test]
fn screens_that_show_one_scene_share_its_textures_and_keep_no_pixels() {
    let Some(r) = renderer() else { return };
    let image = RgbaImage::from_fn(64, 32, |x, y| Rgba([x as u8 * 4, y as u8 * 8, 90, 255]));
    let reloads = Arc::new(AtomicU32::new(0));
    let (counted, again) = (reloads.clone(), image.clone());
    let pixels = Pixels::reloadable(image, move || {
        counted.fetch_add(1, Ordering::Relaxed);
        Some(again.clone())
    });
    let assets = SceneAssets { images: [("photo".to_string(), pixels.clone())].into(), ..Default::default() };
    let scene = scene(json!([{ "id": "p", "type": "image", "image": "photo", "size": [200, 100] }]));
    let runtime = Runtime::new(Arc::new(scene), Values::new());
    let mut first = r.scene_view(&runtime.scene, &assets, runtime.props()).unwrap();
    let uploads = r.uploads_of(&pixels);
    assert!(uploads > 0 && !pixels.resident(), "the pixels stayed in memory after the upload");
    let mut second = r.scene_view(&runtime.scene, &assets, runtime.props()).unwrap();
    draw(r, &mut first, &runtime);
    draw(r, &mut second, &runtime);
    assert_eq!((r.uploads_of(&pixels), reloads.load(Ordering::Relaxed)), (uploads, 0));
    drop((first, second));
    assert_eq!(r.uploads_of(&pixels), 0, "a texture outlived every screen");
    let third = r.scene_view(&runtime.scene, &assets, runtime.props()).unwrap();
    assert_eq!((r.uploads_of(&pixels), reloads.load(Ordering::Relaxed)), (uploads, 1));
    assert!(!pixels.resident());
    drop(third);
    let kept = Pixels::new(RgbaImage::new(4, 4));
    let assets = SceneAssets { images: [("photo".to_string(), kept.clone())].into(), ..Default::default() };
    drop(r.scene_view(&runtime.scene, &assets, runtime.props()).unwrap());
    assert!(kept.resident(), "pixels that cannot be read again were dropped");
}

#[test]
fn released_textures_serve_later_layers_of_the_same_frame() {
    let Some(r) = renderer() else { return };
    let layer = |n: u32| {
        let spin = json!({ "keys": [[0, 0], [10, 360]], "loop": "repeat" });
        json!({ "id": format!("l{n}"), "type": "solid", "color": "#ff8000", "size": [40, 40],
            "transform": { "position": [20 + 30 * n, 50], "rotation": spin },
            "effects": [{ "effect": "blur", "params": { "radius": 4 } }, { "effect": "waves" }] })
    };
    let scene = scene((0..6).map(layer).collect());
    let mut runtime = Runtime::new(Arc::new(scene), Values::new());
    let mut view = r.scene_view(&runtime.scene, &SceneAssets::default(), runtime.props()).unwrap();
    for _ in 0..3 {
        runtime.advance(0.1);
        draw(r, &mut view, &runtime);
    }
    let stats = view.stats();
    assert!(stats.passes >= 18 && stats.created == 0, "{stats:?}");
    assert!(stats.textures <= 8, "six layers held {} textures at once", stats.textures);
}
