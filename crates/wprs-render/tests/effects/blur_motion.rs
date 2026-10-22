use super::*;

#[test]
fn blur_motion_streaks_along_its_angle() {
    let fx =
        |angle: f32| Fx::new("blur_motion", json!({ "angle": angle, "length": 16 })).input(halves(RED, BLUE));
    let Some(across) = fx(0.0).shot() else { return };
    let [red, _, blue] = across.at(32, 16);
    assert!(red > 60 && blue > 60, "streaked edge {:?}", across.at(32, 16));
    across.check(&[((12, 16), RED), ((52, 16), BLUE)]);
    let Some(along) = fx(90.0).shot() else { return };
    along.check(&[((31, 16), RED), ((32, 16), BLUE)]);
}
