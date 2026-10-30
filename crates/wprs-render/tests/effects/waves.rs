use super::*;

#[test]
fn waves_shift_pixels_sideways() {
    let params = json!({ "amplitude": 10, "frequency": 1, "speed": 0, "direction": 90 });
    let Some(out) = Fx::new("waves", params).input(halves(RED, BLUE)).shot() else { return };
    out.check(&[((40, 16), RED), ((50, 16), BLUE), ((10, 16), RED)]);
}
