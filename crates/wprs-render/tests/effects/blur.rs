use super::*;
use wprs_scene::Quality;

#[test]
fn blur_mixes_colors_across_an_edge() {
    let Some(out) = Fx::new("blur", json!({ "radius": 12 })).input(halves(RED, BLUE)).shot() else { return };
    let [red, _, blue] = out.at(32, 16);
    assert!(red > 60 && blue > 60 && red < 250 && blue < 250, "blurred edge {:?}", out.at(32, 16));
    out.check(&[((4, 16), RED), ((60, 16), BLUE)]);
    let lowest = Fx::new("blur", json!({ "radius": 12 })).input(halves(RED, BLUE)).quality(Quality::Q0);
    let Some(sharp) = lowest.shot() else { return };
    sharp.check(&[((31, 16), RED), ((32, 16), BLUE)]);
}
