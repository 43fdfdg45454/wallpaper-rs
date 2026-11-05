use super::kit::{mesh, render, renderer, rgb, scene};
use super::parts::colored;
use serde_json::{Value, json};
use std::sync::Arc;
use wprs_scene::{Quality, Runtime, Values};

const SEE_THROUGH: [[f32; 4]; 2] = [[1.0, 0.0, 0.0, 0.5], [0.0, 1.0, 0.0, 1.0]];

fn layer(depth: Value) -> Value {
    let mut layer = mesh(json!({ "depth": depth }));
    layer.as_object_mut().unwrap().remove("texture");
    layer
}

fn order(depth: Value, z: f32) -> Vec<u16> {
    let mut rt = Runtime::new(Arc::new(scene(layer(depth), colored(z, SEE_THROUGH))), Values::new());
    rt.advance(0.1);
    rt.puppet("fox").unwrap().posed().draws.iter().map(|d| d.part).collect()
}

#[test]
fn with_depth_parts_are_drawn_from_back_to_front_unless_told_not_to() {
    assert_eq!(order(json!({ "scale": 0 }), 4.0), vec![1, 0], "the nearer part comes last");
    assert_eq!(order(json!({ "scale": 0 }), -4.0), vec![0, 1], "the farther part comes first");
    assert_eq!(order(json!({ "scale": 0, "sort": false }), 4.0), vec![0, 1], "the file order stays");
    assert_eq!(order(json!({ "scale": 0 }), 0.0), vec![0, 1], "parts at one depth keep their order");
}

#[test]
fn a_see_through_part_in_front_shows_the_part_behind_it_without_a_halo() {
    let Some(r) = renderer() else { return };
    let draw = |depth: Value| render(r, scene(layer(depth), colored(4.0, SEE_THROUGH)), 0.0, Quality::Q3);
    let sorted = draw(json!({ "scale": 0 }));
    let [red, green, _] = rgb(&sorted, 32, 32);
    assert!(red > 60 && green > 60, "the far part shows through the near one: {:?}", rgb(&sorted, 32, 32));
    let unsorted = draw(json!({ "scale": 0, "sort": false }));
    assert!(
        rgb(&unsorted, 32, 32)[1] < 20,
        "in file order the near part hides it: {:?}",
        rgb(&unsorted, 32, 32)
    );
}
