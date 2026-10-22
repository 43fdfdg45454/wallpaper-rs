use super::*;

#[test]
fn edges_outline_contrast() {
    let Some(out) = Fx::new("edges", json!({})).input(halves(RED, BLUE)).shot() else { return };
    out.check(&[((10, 16), RED), ((54, 16), BLUE), ((31, 16), [0; 3]), ((32, 16), [0; 3])]);
    let Some(sketch) = Fx::new("edges", json!({ "sketch": true })).input(halves(RED, BLUE)).shot() else {
        return;
    };
    sketch.check(&[((10, 16), [255; 3]), ((54, 16), [255; 3]), ((31, 16), [0; 3])]);
}
