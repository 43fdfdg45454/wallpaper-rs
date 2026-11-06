use serde_json::{Value, json};
use std::path::Path;
use wprs_editor::canvas::gizmos;

fn corners(transform: Value) -> Option<[[f64; 2]; 4]> {
    let layer = json!({"id": "card", "type": "solid", "color": "#ffffff", "size": [200, 100], "transform": transform});
    let doc = json!({"scene": {"canvas": {"size": [1000, 1000], "perspective": 1000}, "layers": [layer]}});
    gizmos(Path::new("."), &doc, 0.0)[0].corners
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 0.5 && (a[1] - b[1]).abs() < 0.5
}

#[test]
fn a_lifted_layer_grows_on_the_canvas_as_the_scene_draws_it() {
    assert_eq!(corners(json!({"position": [500, 500]})), None);
    let lifted = corners(json!({"position": [500, 500], "z": 500})).expect("an outline of a lifted layer");
    let wanted = [[300.0, 400.0], [700.0, 400.0], [700.0, 600.0], [300.0, 600.0]];
    assert!(lifted.iter().zip(wanted).all(|(a, b)| near(*a, b)), "{lifted:?}");
}

#[test]
fn a_tilted_layer_narrows_on_the_side_that_leans_away() {
    let tilted =
        corners(json!({"position": [500, 500], "tilt": [0, 30]})).expect("an outline of a tilted layer");
    let (left, right) = ((tilted[3][1] - tilted[0][1]).abs(), (tilted[2][1] - tilted[1][1]).abs());
    assert!((left - right).abs() > 5.0, "both sides stay {left} and {right} high: {tilted:?}");
    let turned = corners(json!({"position": [500, 500], "tilt": [0, 30], "rotation": 90})).unwrap();
    assert!(!near(turned[0], tilted[0]), "the outline did not turn with the layer");
}
