use super::*;

fn masked(effect: &str, params: Value) -> Value {
    json!({ "effect": effect, "params": params, "mask": { "texture": "left", "channel": "r" } })
}

fn run(list: Value, input: RgbaImage) -> Option<(Shot, u32)> {
    let mut run = Fx::chain(list).input(input).image("left", halves([255; 3], [0; 3]), true).run()?;
    Some((run.shot(), run.stats().passes))
}

#[test]
fn mixed_masks_stay_on_their_side_when_the_chain_fuses() {
    let gray = masked("color_adjust", json!({ "saturation": 0 }));
    let noop = json!({ "effect": "color_adjust", "params": {} });
    let (Some((alone, _)), Some((fused, _))) =
        (run(json!([gray.clone()]), solid(RED)), run(json!([gray, noop.clone()]), solid(RED)))
    else {
        return;
    };
    assert_eq!(fused.at(56, 16), RED, "{:?}", fused.at(56, 16));
    assert_eq!(fused.at(8, 16), alone.at(8, 16));
    assert_ne!(fused.at(8, 16), RED);
    let blocky = masked("pixelate", json!({ "size": 16 }));
    let (Some((a, _)), Some((b, _))) =
        (run(json!([blocky.clone()]), ramp()), run(json!([blocky, noop]), ramp()))
    else {
        return;
    };
    assert!(a.diff(&b) < 1.0, "{}", a.diff(&b));
    assert_eq!(b.at(56, 16), ramp().get_pixel(56, 16).0[..3]);
}

fn same_name(texture: &str) -> Option<Shot> {
    let half = json!({ "id": "x", "type": "solid", "color": "#ffffff", "size": [32, 32], "visible": false,
        "transform": { "position": [48, 16] } });
    let green = json!({ "effect": "tint", "params": { "color": "#00ff00", "blend": "normal" },
        "mask": { "layer": "x" } });
    let blue = json!({ "effect": "tint", "params": { "color": "#0000ff", "blend": "normal" },
        "mask": { "texture": texture, "channel": "r" } });
    let clear = RgbaImage::from_fn(W, H, |x, _| Rgba(if x < W / 2 { [255; 4] } else { [0; 4] }));
    Fx::chain(json!([green, blue])).input(solid(RED)).layer(half).image(texture, clear, true).shot()
}

#[test]
fn a_layer_mask_and_a_picture_mask_of_one_name_stay_apart() {
    let (Some(apart), Some(shared)) = (same_name("y"), same_name("x")) else { return };
    assert!(apart.diff(&shared) < 1.0, "{}", apart.diff(&shared));
    assert_eq!(shared.at(56, 16), [0, 255, 0]);
}
