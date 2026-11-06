use super::*;

const WIDE: u32 = 256;

fn stripes(every: u32) -> RgbaImage {
    RgbaImage::from_fn(WIDE, H, |x, _| Rgba(if (x / every).is_multiple_of(2) { [255; 4] } else { [0, 0, 0, 255] }))
}

fn tint(color: &str, mask: &str) -> Value {
    json!({ "effect": "tint", "params": { "color": color, "blend": "normal" },
        "mask": { "layer": mask, "channel": "r" } })
}

fn wide(id: &str, effects: Value) -> Value {
    json!({ "id": id, "type": "image", "image": id, "size": [WIDE, H], "effects": effects,
        "transform": { "position": [W / 2, H / 2] } })
}

fn shot(tints: Value, also: Value) -> Option<Shot> {
    let mut fx = Fx::chain(json!([])).image("big", RgbaImage::from_pixel(WIDE, H, Rgba([255, 0, 0, 255])), true);
    for (id, every) in [("m", 8), ("n", 16)] {
        let mut mask = wide(id, json!([]));
        mask["visible"] = json!(false);
        fx = fx.image(id, stripes(every), true).layer(mask);
    }
    let fx = fx.layer(wide("big", tints));
    fx.layer(json!({ "id": "other", "type": "image", "image": "big", "size": [1, 1], "effects": also })).shot()
}

#[test]
fn a_layer_mask_wider_than_the_screen_lines_up_when_only_its_visible_part_is_drawn() {
    let shift = json!([{ "effect": "displace", "params": { "map": { "layer": "m" } } }]);
    let tints = json!([tint("#00ff00", "m")]);
    let (Some(cropped), Some(whole)) = (shot(tints.clone(), json!([])), shot(tints, shift)) else { return };
    for x in (4..W).step_by(8) {
        let stripe = ((x + (WIDE - W) / 2) / 8).is_multiple_of(2);
        let want = if stripe { [0, 255, 0] } else { [255, 0, 0] };
        assert_eq!(cropped.at(x, H / 2), want, "column {x}");
    }
    assert!(cropped.diff(&whole) < 1.0, "{}", cropped.diff(&whole));
}

#[test]
fn two_cropped_layer_masks_in_one_fused_pass_keep_their_own_place() {
    let tints = json!([tint("#00ff00", "m"), tint("#0000ff", "n")]);
    let bound = |id: &str| json!({ "effect": "displace", "params": { "map": { "layer": id } } });
    let (Some(cropped), Some(whole)) =
        (shot(tints.clone(), json!([])), shot(tints, json!([bound("m"), bound("n")])))
    else {
        return;
    };
    assert!(cropped.diff(&whole) < 1.0, "{}", cropped.diff(&whole));
    assert_eq!((cropped.at(4, H / 2), cropped.at(20, H / 2)), ([0, 0, 255], [0, 255, 0]));
}
