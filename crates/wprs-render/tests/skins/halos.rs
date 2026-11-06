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
fn a_see_through_part_in_front_shows_the_part_behind_it_without_a_halo_in_any_order() {
    let Some(r) = renderer() else { return };
    let draw = |depth: Value| render(r, scene(layer(depth), colored(4.0, SEE_THROUGH)), 0.0, Quality::Q3);
    let sorted = draw(json!({ "scale": 0 }));
    let [red, green, _] = rgb(&sorted, 32, 32);
    assert!(red > 60 && green > 60, "the far part shows through the near one: {:?}", rgb(&sorted, 32, 32));
    let unsorted = draw(json!({ "scale": 0, "sort": false }));
    let [red, green, _] = rgb(&unsorted, 32, 32);
    assert!(red > 60 && green > 60, "soft parts never hide what lies behind: {:?}", rgb(&unsorted, 32, 32));
    let solid = [[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]];
    let near =
        render(r, scene(layer(json!({ "scale": 0, "sort": false })), colored(4.0, solid)), 0.0, Quality::Q3);
    assert!(rgb(&near, 32, 32)[0] > 200 && rgb(&near, 32, 32)[1] < 20, "{:?}", rgb(&near, 32, 32));
}

#[test]
fn at_one_depth_the_part_drawn_later_covers_the_soft_pixels_of_the_one_before() {
    let Some(r) = renderer() else { return };
    let draw =
        |colors| render(r, scene(layer(json!({ "scale": 0 })), colored(0.0, colors)), 0.0, Quality::Q3);
    let covered = draw(SEE_THROUGH);
    let [red, green, _] = rgb(&covered, 32, 32);
    assert!(red < 20 && green > 200, "the soft back part stays under the front one: {:?}", [red, green]);
    let over = draw([[0.0, 1.0, 0.0, 1.0], [1.0, 0.0, 0.0, 0.5]]);
    let [red, green, _] = rgb(&over, 32, 32);
    assert!(red > 60 && green > 60, "a soft front part still shows what lies under it: {:?}", [red, green]);
}
