mod fonts;
mod styles;
mod text;
mod time;

use super::*;
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame};
use wprs_render::Pixels;
use wprs_scene::Wake;

fn looks_like(name: &str, pixels: &[u8]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/looks/{name}.png"));
    let shot = image::RgbImage::from_fn(SIZE.0, SIZE.1, |x, y| image::Rgb(at(pixels, x, y)));
    if std::env::var_os("WPRS_BLESS").is_some() {
        return shot.save(&path).unwrap();
    }
    let reference = image::open(&path).unwrap().into_rgb8();
    let off = shot.pixels().zip(reference.pixels()).filter(|(a, b)| !near(a.0, b.0, 24)).count();
    assert!(off <= 25, "{name}: {off} pixels differ from tests/looks/{name}.png");
}

fn picture(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Arc<Pixels> {
    Pixels::new(RgbaImage::from_fn(w, h, |x, y| Rgba(f(x, y))))
}

fn sheet(asset: Value, more: Value) -> Value {
    let mut layer = json!({ "id": "i", "type": "image", "image": "rb", "size": [100, 50] });
    layer.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    json!({ "canvas": canvas("#000000"), "assets": { "rb": asset }, "layers": [layer] })
}

#[test]
fn tints_brightness_parents_and_weak_masks() {
    let Some(r) = renderer() else { return };
    let white = picture(4, 4, |_, _| [255; 4]);
    let layers = json!([
        { "id": "img", "type": "image", "image": "w", "size": [20, 50], "color": "#ff0000", "transform": { "position": [10, 25] } },
        { "id": "dim", "type": "solid", "color": "#ffffff", "brightness": 0.5, "size": [20, 50], "transform": { "position": [30, 25] } },
        { "id": "g", "type": "group", "color": "#00ff00", "layers": [
            { "id": "in", "type": "solid", "color": "#ffffff", "size": [20, 50], "transform": { "position": [50, 25] } }] },
        { "id": "kid", "type": "solid", "color": "#0000ff", "size": [10, 10], "parent": "box", "transform": { "position": [12, 0] } },
        { "id": "box", "type": "solid", "color": "#ffffff", "size": [10, 10], "transform": { "position": [70, 10] } },
        { "id": "ghost", "type": "solid", "color": "#ffffff", "size": [10, 10], "parent": "gone", "transform": { "position": [0, 20] } },
        { "id": "gone", "type": "solid", "color": "#ffffff", "size": [10, 10], "visible": false, "transform": { "position": [70, 20] } },
        { "id": "half", "type": "solid", "color": "#ff0000", "size": [10, 10], "transform": { "position": [90, 40] },
          "mask": { "shape": { "ellipse": { "size": [1, 1] } }, "strength": 0.5 } }
    ]);
    let pixels = Shot::new(r, canvas("#000000"), layers, &[("w", white)]).pixels(r);
    check(&pixels, &[((10, 25), [255, 0, 0]), ((30, 25), [188, 188, 188]), ((50, 25), [0, 255, 0])]);
    check(&pixels, &[((82, 10), [0, 0, 255]), ((70, 10), [255, 255, 255]), ((70, 40), [0, 0, 0])]);
    check(&pixels, &[((88, 38), [188, 0, 0])]);
    looks_like("tints", &pixels);
}

#[test]
fn pictures_filter_repeat_mirror_and_shrink() {
    let Some(r) = renderer() else { return };
    let ends = picture(2, 1, |x, _| if x == 0 { [0, 0, 0, 255] } else { [255; 4] });
    let rb = picture(20, 10, |x, _| if x < 10 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
    let shot = |asset: Value, image: &Arc<Pixels>| {
        Shot::with(r, sheet(asset, json!({})), &[("rb", image.clone())]).pixels(r)
    };
    let soft = at(&shot(json!({ "path": "rb.png" }), &ends), 50, 25)[0];
    assert!(soft > 100 && soft < 250, "{soft}");
    check(
        &shot(json!({ "path": "rb.png", "filter": "nearest" }), &ends),
        &[((49, 25), [0; 3]), ((51, 25), [255; 3])],
    );
    let repeat = shot(json!({ "path": "rb.png", "wrap": "repeat" }), &rb);
    check(&repeat, &[((25, 25), [255, 0, 0]), ((35, 25), [0, 0, 255]), ((85, 5), [255, 0, 0])]);
    let mirror = shot(json!({ "path": "rb.png", "wrap": "mirror" }), &rb);
    check(&mirror, &[((5, 25), [255, 0, 0]), ((25, 25), [0, 0, 255]), ((35, 25), [255, 0, 0])]);
    let tiles = json!({ "path": "rb.png", "wrap": "repeat", "filter": "nearest" });
    let both = json!([{ "id": "r", "type": "image", "image": "rb", "size": [50, 50], "transform": { "position": [25, 25] } },
        { "id": "m", "type": "image", "image": "m", "size": [50, 50], "transform": { "position": [75, 25] } }]);
    let text = json!({ "canvas": canvas("#000000"), "assets": { "rb": tiles, "m": { "path": "m.png", "wrap": "mirror" } },
        "layers": both });
    let lit = picture(20, 10, |x, y| [x as u8 * 12, y as u8 * 25, 255 - x as u8 * 12, 255]);
    looks_like("wraps", &Shot::with(r, text, &[("rb", lit.clone()), ("m", lit)]).pixels(r));
    let checker = picture(64, 64, |x, y| if (x + y) % 2 == 0 { [0, 0, 0, 255] } else { [255; 4] });
    let small = |mipmaps: Value| {
        let mut asset = json!({ "path": "c.png" });
        if !mipmaps.is_null() {
            asset["mipmaps"] = mipmaps;
        }
        let text = json!({ "canvas": canvas("#000000"), "assets": { "c": asset },
            "layers": [{ "id": "c", "type": "image", "image": "c", "size": [20, 20] }] });
        let pixels = Shot::with(r, text, &[("c", checker.clone())]).pixels(r);
        (42..58)
            .flat_map(|x| (17..33).map(move |y| (x, y)))
            .map(|(x, y)| at(&pixels, x, y)[0])
            .collect::<Vec<_>>()
    };
    for mipmaps in [json!(true), Value::Null] {
        let grey = small(mipmaps.clone());
        assert!(grey.iter().all(|v| (150..225).contains(v)), "{mipmaps}: {grey:?}");
    }
    assert!(small(json!(false)).iter().any(|v| !(150..225).contains(v)));
}

#[test]
fn sprite_sheets_and_animated_pictures_play_their_frames() {
    let Some(r) = renderer() else { return };
    let rb = picture(20, 10, |x, _| if x < 10 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
    let asset = json!({ "path": "rb.png", "atlas": [1, 2], "fps": 2 });
    let mut shot = Shot::with(r, sheet(asset.clone(), json!({})), &[("rb", rb.clone())]);
    check(&shot.pixels(r), &[((50, 25), [255, 0, 0]), ((95, 25), [255, 0, 0])]);
    assert_eq!(shot.runtime.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3).wake, Wake::At(0.5));
    shot.runtime.seek(0.75);
    let second = shot.pixels(r);
    check(&second, &[((5, 25), [0, 0, 255]), ((95, 25), [0, 0, 255])]);
    looks_like("sheet", &second);
    let picked = sheet(asset, json!({ "playback": { "frame": 1, "playing": false } }));
    check(&Shot::with(r, picked, &[("rb", rb)]).pixels(r), &[((50, 25), [0, 0, 255])]);
    let mut gif = Vec::new();
    let mut encoder = GifEncoder::new(&mut gif);
    encoder.set_repeat(Repeat::Infinite).unwrap();
    for (color, ms) in [([255, 0, 0, 255], 100), ([0, 0, 255, 255], 300)] {
        let image = RgbaImage::from_pixel(4, 2, Rgba(color));
        encoder.encode_frame(Frame::from_parts(image, 0, 0, Delay::from_numer_denom_ms(ms, 1))).unwrap();
    }
    drop(encoder);
    let (atlas, durations) = wprs_render::animation(&gif).unwrap();
    assert_eq!((atlas.dimensions(), durations), ((8, 2), vec![0.1, 0.3]));
    let text = sheet(json!({ "path": "rb.gif", "animated": true }), json!({}));
    let (atlas, sheets) = (Pixels::new(atlas), [("rb".to_string(), vec![0.1, 0.3])].into());
    let mut shot = Shot::with(r, text, &[("rb", atlas.clone())]);
    let assets = SceneAssets { images: [("rb".to_string(), atlas)].into(), sheets, ..Default::default() };
    shot.view = r.scene_view(&shot.runtime.scene, &assets, shot.runtime.props()).unwrap();
    shot.runtime.set_sheet("rb", vec![0.1, 0.3]);
    shot.runtime.seek(0.2);
    check(&shot.pixels(r), &[((50, 25), [0, 0, 255])]);
    shot.runtime.seek(0.45);
    check(&shot.pixels(r), &[((50, 25), [255, 0, 0])]);
}
