use super::{Shot, UP, at, files, renderer, scene, wall};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_ktx2::{Codec, Options};
use wprs_render::Pixels;
use wprs_scene::Quality;

fn edge(split: u32) -> RgbaImage {
    RgbaImage::from_fn(30, 18, |x, _| Rgba(if x < split { [0, 0, 0, 255] } else { [255; 4] }))
}

fn packed(image: &RgbaImage) -> Arc<Pixels> {
    let options = Options { codec: Codec::Uastc, quality: 90, srgb: true, mips: false, repeat: false };
    Pixels::ktx2(wprs_ktx2::encode(image, &options).unwrap()).unwrap()
}

fn shot(layers: &Value, canvas: &Value, swap: &[(&str, Arc<Pixels>)]) -> Option<RgbaImage> {
    let r = renderer()?;
    let mut assets = files(UP);
    swap.iter().for_each(|(id, p)| drop(assets.images.insert(id.to_string(), p.clone())));
    Some(Shot::new(r, scene(layers.clone(), canvas.clone()), &assets).pixels(r, Quality::Q3))
}

fn bright(image: &RgbaImage, channel: usize) -> usize {
    (0..160).filter(|x| at(image, *x, 45)[channel] > 128).count()
}

fn spot(texture: Value) -> Value {
    json!({ "id": "spot", "type": "light", "light": "spot", "cone": [80, 90], "radius": 400, "z": 40,
        "intensity": 2, "transform": { "position": [80, 45], "tilt": [89, 0] }, "texture": texture })
}

fn padded() -> bool {
    renderer().is_some_and(|r| r.ktx2_target().compressed())
}

fn same(layers: Value, canvas: Value, id: &str, picture: RgbaImage, channel: usize) {
    let plain = shot(&layers, &canvas, &[(id, Pixels::new(picture.clone()))]);
    let packed = shot(&layers, &canvas, &[(id, packed(&picture))]);
    let (Some(plain), Some(packed)) = (plain, packed) else { return };
    let (a, b) = (bright(&plain, channel), bright(&packed, channel));
    assert!(a.abs_diff(b) <= 2, "{id} in {layers}: {a} bright with a png, {b} with a padded ktx2");
    assert!((10..150).contains(&a), "{id} in {layers}: the edge was not on the row: {a}");
}

#[test]
fn padded_ktx2_masks_maps_and_projections_line_up_like_their_pngs() {
    if !padded() {
        return eprintln!("skipping: the card reads no compressed KTX2 class");
    }
    let dark = json!({ "ambient": "#000000" });
    let masked = json!({ "id": "m", "type": "image", "image": "wall", "size": [160, 90],
        "transform": { "position": [80, 45] }, "mask": { "texture": "mask" } });
    same(json!([masked]), dark.clone(), "mask", edge(20), 0);
    let tinted = json!({ "id": "t", "type": "image", "image": "wall", "size": [160, 90],
        "transform": { "position": [80, 45] },
        "effects": [{ "effect": "tint", "params": { "color": "#ff0000" }, "mask": { "texture": "mask" } }] });
    let tint = |image: &RgbaImage| (0..160).filter(|x| at(image, *x, 45)[0] < 100).count();
    let layers = json!([tinted]);
    let plain = shot(&layers, &dark, &[("mask", Pixels::new(edge(20)))]);
    let packed_mask = shot(&layers, &dark, &[("mask", packed(&edge(20)))]);
    if let (Some(plain), Some(packed_mask)) = (plain, packed_mask) {
        assert!(tint(&plain).abs_diff(tint(&packed_mask)) <= 2, "{} {}", tint(&plain), tint(&packed_mask));
    }
    let glow = json!([wall(json!({ "emissive": 1.0, "emissive_map": "mask" }))]);
    same(glow.clone(), dark.clone(), "mask", edge(20), 0);
    let flat = RgbaImage::from_pixel(30, 18, Rgba([200, 200, 200, 255]));
    let odd = shot(&glow, &dark, &[("wall", packed(&flat)), ("mask", Pixels::new(edge(20)))]);
    let even = shot(&glow, &dark, &[("mask", Pixels::new(edge(20)))]);
    if let (Some(odd), Some(even)) = (odd, even) {
        let (a, b) = (bright(&even, 0), bright(&odd, 0));
        assert!(a.abs_diff(b) <= 2, "a png map under a padded picture: {a} {b}");
    }
    same(json!([wall(json!({})), spot(json!("mask"))]), dark, "mask", edge(15), 0);
}

#[test]
fn a_padded_ktx2_reflection_is_read_where_its_png_is() {
    if !padded() {
        return;
    }
    let env =
        RgbaImage::from_fn(30, 18, |x, _| Rgba(if x < 16 { [0, 0, 255, 255] } else { [255, 0, 0, 255] }));
    let layers = json!([wall(json!({ "reflection": 1.0, "roughness": 0.0, "env": "env" }))]);
    let canvas = json!({ "ambient": "#ffffff" });
    let plain = shot(&layers, &canvas, &[("env", Pixels::new(env.clone()))]);
    let packed = shot(&layers, &canvas, &[("env", packed(&env))]);
    let (Some(plain), Some(packed)) = (plain, packed) else { return };
    let ([pb, _, pr, _], [kb, _, kr, _]) = (at(&plain, 80, 45), at(&packed, 80, 45));
    assert!(pb.abs_diff(kb) < 30 && pr.abs_diff(kr) < 30, "png {pb} {pr}, ktx2 {kb} {kr}");
}
