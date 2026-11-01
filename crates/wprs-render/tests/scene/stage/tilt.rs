use super::*;

fn white(pixels: &[u8], x: u32, y: u32) -> bool {
    at(pixels, x, y)[0] > 128
}

fn column(pixels: &[u8], x: u32) -> usize {
    (0..50).filter(|&y| white(pixels, x, y)).count()
}

fn square(more: Value) -> Value {
    let mut layer = json!({ "id": "s", "type": "solid", "color": "#ffffff", "size": [40, 40] });
    layer.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    json!([layer])
}

#[test]
fn tilted_layers_lean_in_depth_with_their_masks() {
    let Some(r) = renderer() else { return };
    let flat = Shot::new(r, canvas("#000000"), square(json!({})), &[]).pixels(r);
    assert_eq!((column(&flat, 32), column(&flat, 68)), (40, 40));
    let tilted = json!({ "transform": { "tilt": [0, 50] } });
    let world = json!({ "size": [100, 50], "background": "#000000", "perspective": 120 });
    let leaned = Shot::new(r, world.clone(), square(tilted.clone()), &[]).pixels(r);
    let (near, far) = (column(&leaned, 40), column(&leaned, 58));
    assert!(near > far + 4, "the right edge goes away: {near} against {far}");
    assert!(!white(&leaned, 33, 25) && white(&leaned, 50, 25), "the square narrows");
    let mut masked = tilted.clone();
    masked["mask"] = json!({ "shape": { "ellipse": { "size": [1, 1] } } });
    let round = Shot::new(r, world.clone(), square(masked), &[]).pixels(r);
    assert!(white(&round, 50, 25) && !white(&round, 40, 7), "the mask leans with the layer");
    let grouped = json!([{ "id": "g", "type": "group", "transform": { "tilt": [0, 50], "position": [50, 25] },
        "layers": [{ "id": "s", "type": "solid", "color": "#ffffff", "size": [40, 40], "transform": { "position": [0, 0] } }] }]);
    let group = Shot::new(r, world.clone(), grouped, &[]).pixels(r);
    assert!((40..60).all(|x| column(&group, x).abs_diff(column(&leaned, x)) <= 2));
    let close =
        Shot::new(r, world, square(json!({ "size": [20, 20], "transform": { "z": 60 } })), &[]).pixels(r);
    assert_eq!(column(&close, 50), 40, "half way to the viewer it looks twice as large");
}
