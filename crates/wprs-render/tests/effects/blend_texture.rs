use super::*;

fn dots() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| Rgba(if (x / 8 + y / 8) % 2 == 0 { [255, 64, 0, 255] } else { [0, 128, 255, 255] }))
}

#[test]
fn blend_texture_mixes_pictures_and_layers_with_a_blend_mode() {
    let with = |params: Value| Fx::new("blend_texture", params).input(solid([255; 3])).image("dots", dots(), false);
    let Some(multiply) = with(json!({ "texture": "dots" })).shot() else { return };
    multiply.check(&[((2, 2), [255, 64, 0]), ((10, 2), [0, 128, 255])]);
    let Some(tiled) = with(json!({ "texture": "dots", "scale": [0.5, 0.5], "amount": 0.5 })).shot() else { return };
    assert!(tiled.at(2, 2) != multiply.at(2, 2) || tiled.at(5, 2) != multiply.at(5, 2));
    let Some(screen) = Fx::new("blend_texture", json!({ "texture": "dots", "blend": "screen" }))
        .image("dots", dots(), false)
        .input(solid([0; 3]))
        .shot()
    else {
        return;
    };
    screen.check(&[((2, 2), [255, 64, 0]), ((10, 2), [0, 128, 255])]);
    let spot = json!({ "id": "spot", "type": "shape", "shape": { "ellipse": { "size": [20, 20] } }, "fill": "#00ff00",
        "visible": false, "transform": { "position": [48, 16] } });
    let fed = json!({ "texture": { "layer": "spot" }, "blend": "normal" });
    let Some(layered) = Fx::new("blend_texture", fed).input(solid([255; 3])).layer(spot).shot() else { return };
    layered.check(&[((48, 16), [0, 255, 0]), ((16, 16), [255; 3]), ((48, 2), [255; 3])]);
    Shot::sheet(&[multiply, tiled, screen, layered]).looks_like("blend_texture");
}
