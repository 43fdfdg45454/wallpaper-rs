use super::*;

#[test]
fn perspective_maps_the_layer_onto_its_corners() {
    let corners = |tl: [f32; 2], tr: [f32; 2]| json!({ "top_left": tl, "top_right": tr });
    let Some(same) = Fx::new("perspective", corners([0.0, 0.0], [1.0, 0.0])).input(ramp()).shot() else {
        return;
    };
    assert!(same.diff(&Shot(ramp())) < 0.5);
    let fx = Fx::new("perspective", corners([0.25, 0.0], [0.75, 0.0])).input(solid(RED));
    let Some(keystone) = fx.shot() else { return };
    keystone.check(&[((4, 1), [0; 3]), ((60, 1), [0; 3]), ((32, 1), RED), ((2, 30), RED), ((61, 30), RED)]);
}
