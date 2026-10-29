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

#[test]
fn blur_motion_follows_the_layer_velocity() {
    let fx = Fx::new("blur_motion", json!({ "follow": true, "length": 32, "shutter": 0.1 })).input(halves(RED, BLUE));
    let Some(mut run) = fx.run() else { return };
    let still = run.shot();
    still.check(&[((31, 16), RED), ((32, 16), BLUE)]);
    let across = run.shot_edited((W, H), |frame| frame.effects[0].velocity = [160.0, 0.0]);
    let [red, _, blue] = across.at(32, 16);
    assert!(red > 60 && blue > 60, "streaked edge {:?}", across.at(32, 16));
    across.check(&[((20, 16), RED), ((44, 16), BLUE)]);
    let down = run.shot_edited((W, H), |frame| frame.effects[0].velocity = [0.0, 160.0]);
    down.check(&[((31, 16), RED), ((32, 16), BLUE)]);
}
