use super::*;

#[test]
fn adjustment_layers_capture_a_region_and_draw_it_again() {
    let Some(r) = renderer() else { return };
    let mirror = json!([{ "id": "m", "type": "adjustment", "size": [20, 20], "redraw": true,
        "transform": { "position": [50, 25], "scale": [-1, 1] } }]);
    let mirrored = halved(r, json!({}), mirror).pixels(r);
    check(&mirrored, &[((45, 25), BLUE), ((55, 25), RED), ((30, 25), RED), ((70, 25), BLUE), ((45, 5), RED)]);
    let lens = json!([{ "id": "z", "type": "adjustment", "size": [10, 10], "redraw": true,
        "transform": { "position": [48, 25], "scale": 3 } }]);
    let zoomed = halved(r, json!({}), lens).pixels(r);
    check(&zoomed, &[((40, 25), RED), ((52, 25), RED), ((57, 25), BLUE), ((30, 25), RED), ((66, 25), BLUE)]);
    let grey = json!([{ "id": "i", "type": "adjustment", "size": [20, 20],
        "transform": { "position": [20, 25] }, "effects": [{ "effect": "color_adjust", "params": { "saturation": 0 } }] }]);
    let part = halved(r, json!({}), grey).pixels(r);
    let [cr, cg, cb] = at(&part, 20, 25);
    assert!(cr == cg && cg == cb && cr < 200, "the captured part turns grey: {:?}", [cr, cg, cb]);
    check(&part, &[((5, 25), RED), ((20, 5), RED), ((80, 25), BLUE)]);
    let whole =
        json!([{ "id": "w", "type": "adjustment", "redraw": true, "transform": { "scale": [-1, 1] } }]);
    check(&halved(r, json!({}), whole).pixels(r), &[((10, 25), BLUE), ((90, 25), RED)]);
}

#[test]
fn a_capture_redraws_the_whole_canvas_whenever_something_changes() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "m", "type": "solid", "color": "#ffffff", "size": [4, 4],
          "transform": { "position": { "keys": [[0, [10, 10]], [1, [12, 10]]] } } },
        { "id": "z", "type": "adjustment", "size": [10, 10], "redraw": true, "transform": { "position": [80, 25] } }]);
    let mut shot = halved(r, json!({}), layers);
    shot.pixels(r);
    shot.runtime.seek(0.5);
    shot.pixels(r);
    assert_eq!(shot.view.stats().damage, 100);
    shot.pixels(r);
    assert_eq!(shot.view.stats().damage, 0, "a still scene with a capture stays clean");
}
