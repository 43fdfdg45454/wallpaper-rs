use super::*;
use wprs_ktx2::{Codec, Options, Shape, encode_frames, encode_hdr};

fn options() -> Options {
    Options { codec: Codec::Uastc, quality: 90, srgb: true, mips: false, repeat: false }
}

fn solid(c: [u8; 3]) -> RgbaImage {
    RgbaImage::from_pixel(16, 16, Rgba([c[0], c[1], c[2], 255]))
}

fn shot(r: &Renderer, asset: Value, layer: Value, bytes: Vec<u8>) -> Vec<u8> {
    let mut layer = layer;
    layer["id"] = json!("p");
    layer["type"] = json!("image");
    layer["image"] = json!("p");
    layer["size"] = json!([100, 50]);
    let text = json!({ "canvas": canvas("#000000"), "assets": { "p": asset }, "layers": [layer] });
    Shot::with(r, text, &[("p", Pixels::ktx2(bytes).unwrap())]).pixels(r)
}

#[test]
fn hdr_ktx2_pictures_draw_on_every_card_and_clamp_on_a_plain_canvas() {
    let Some(r) = renderer() else { return };
    let pixels: Vec<f32> = (0..64).flat_map(|i| [if i < 32 { 4.0 } else { 0.25 }, 0.5, 0.1, 1.0]).collect();
    let bytes = encode_hdr((8, 8), &pixels, &options()).unwrap();
    eprintln!("hdr targets: {:?}", r.ktx2_hdr_targets());
    for target in r.ktx2_targets() {
        r.force_ktx2(Some(target));
        let seen = shot(r, json!({ "path": "p.ktx2" }), json!({}), bytes.clone());
        let lit = r.ktx2_hdr_target();
        assert!(near(at(&seen, 50, 6), [255, 188, 89], 16), "{lit:?}: {:?}", at(&seen, 50, 6));
        assert!(near(at(&seen, 50, 44), [137, 188, 89], 16), "{lit:?}: {:?}", at(&seen, 50, 44));
    }
    r.force_ktx2(None);
}

#[test]
fn ktx2_arrays_are_sprite_sheets_and_cubes_are_panoramas() {
    let Some(r) = renderer() else { return };
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255]];
    let frames: Vec<RgbaImage> = colors.iter().map(|c| solid(*c)).collect();
    let bytes = encode_frames(&frames, &options(), Shape::Frames).unwrap();
    for target in r.ktx2_targets() {
        r.force_ktx2(Some(target));
        let asset = json!({ "path": "p.ktx2", "atlas": [2, 3] });
        let seen = shot(r, asset, json!({ "playback": { "frame": 4 } }), bytes.clone());
        assert!(near(at(&seen, 50, 25), [0, 255, 255], 24), "{target:?}: {:?}", at(&seen, 50, 25));
    }
    r.force_ktx2(None);
    let faces: Vec<RgbaImage> = (0..6u8).map(|f| solid([f * 40, 255 - f * 40, 100])).collect();
    let cube = encode_frames(&faces, &options(), Shape::Cube).unwrap();
    let seen = shot(r, json!({ "path": "p.ktx2" }), json!({}), cube);
    assert!(near(at(&seen, 50, 25), [160, 95, 100], 12), "{:?}", at(&seen, 50, 25));
    let mut assets = SceneAssets::default();
    let asset: wprs_scene::Asset =
        serde_json::from_value(json!({ "path": "a.ktx2", "animated": true })).unwrap();
    let mut pictures = wprs_render::Pictures::default();
    let timed = wprs_ktx2::timed(&bytes, 0.5);
    pictures.add::<image::ImageError>((&mut assets, "a", &asset), || Ok(timed), |_| unreachable!()).unwrap();
    assert_eq!((assets.sheets["a"].clone(), assets.images["a"].size()), (vec![0.5; 5], (48, 32)));
}
