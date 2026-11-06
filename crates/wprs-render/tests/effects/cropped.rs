use super::*;

fn rows() -> RgbaImage {
    RgbaImage::from_fn(32, 32, |_, y| Rgba(if (y / 3) % 2 == 0 { [255; 4] } else { [0, 0, 0, 255] }))
}

fn waved(x: i32, amplitude: f32) -> Option<Shot> {
    let waves = json!({ "effect": "waves", "params": { "amplitude": amplitude, "frequency": 1.5, "speed": 0 } });
    let layer = json!({ "id": "w", "type": "image", "image": "rows", "size": [32, 32],
        "transform": { "position": [x, 16] }, "effects": [waves] });
    Fx::chain(json!([])).input(solid([0; 3])).image("rows", rows(), false).layer(layer).shot()
}

#[test]
fn waves_keep_their_pattern_on_the_layer_when_part_of_it_is_off_screen() {
    let (Some(whole), Some(cut), Some(flat)) = (waved(32, 3.0), waved(8, 3.0), waved(32, 0.0)) else { return };
    let (mut sum, mut n) = (0u64, 0u64);
    for y in 0..H {
        for x in 4..22 {
            let (a, b) = (whole.at(x + 24, y), cut.at(x, y));
            sum += u64::from(a[0].abs_diff(b[0]));
            n += 1;
        }
    }
    let mean = sum as f32 / n as f32;
    assert!(mean < 2.0, "the visible part of the layer waves differently: {mean}");
    assert!(whole.diff(&flat) > 1.0, "the waves do not move the layer");
}
