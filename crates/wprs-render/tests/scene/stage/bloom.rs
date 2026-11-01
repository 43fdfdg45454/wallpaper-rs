use super::*;

fn spot(r: &Renderer, canvas: Value) -> Shot {
    let dot = json!([{ "id": "d", "type": "solid", "color": "#ffffff", "size": [6, 6] }]);
    let mut world = json!({ "size": [100, 50], "background": "#000000" });
    world.as_object_mut().unwrap().extend(canvas.as_object().unwrap().clone());
    Shot::new(r, world, dot, &[])
}

#[test]
fn scene_bloom_glows_with_its_tint_and_its_own_set_at_the_highest_levels() {
    let Some(r) = renderer() else { return };
    let plain = spot(r, json!({})).pixels(r);
    assert_eq!(at(&plain, 56, 25), [0; 3]);
    let glow = spot(r, json!({ "bloom": { "threshold": 0.5, "strength": 2, "radius": 8 } })).pixels(r);
    let near = at(&glow, 56, 25);
    assert!(near[0] > 10 && near[0] == near[1], "{near:?}");
    let red = json!({ "bloom": { "threshold": 0.5, "strength": 2, "radius": 8, "tint": "#ff0000" } });
    let tinted = spot(r, red).pixels(r);
    let near = at(&tinted, 56, 25);
    assert!(near[0] > 10 && near[1] < 3 && near[2] < 3, "{near:?}");
    let encoded = |tint: &str| {
        let bloom = json!({ "threshold": 0.5, "strength": 2, "radius": 8, "tint": tint });
        at(&spot(r, json!({ "blend_space": "srgb", "bloom": bloom })).pixels(r), 54, 25)[0]
    };
    let (white, grey) = (f32::from(encoded("#ffffff")), f32::from(encoded("#808080")));
    assert!(white > 20.0 && (grey / white - 0.5).abs() < 0.1, "{grey} of {white}");
    let hdr = json!({ "bloom": { "threshold": 0.5, "strength": 0, "radius": 8, "hdr": { "threshold": 0.5, "strength": 4 } } });
    let mut shot = spot(r, hdr);
    let high = at(&shot.pixels(r), 56, 25)[0];
    let middle = at(&at_level(&mut shot, r, Quality::Q2), 56, 25)[0];
    assert!(high > 10 && middle > 10, "{high} {middle}");
    shot.runtime.set_battery(true);
    assert_eq!(at(&at_level(&mut shot, r, Quality::Q2), 56, 25)[0], 0, "on battery Q2 keeps the plain set");
    assert_eq!(at(&at_level(&mut shot, r, Quality::Q1), 56, 25)[0], 0);
}
