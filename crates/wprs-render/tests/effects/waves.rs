use super::*;

#[test]
fn waves_shift_pixels_sideways() {
    let params = json!({ "amplitude": 10, "frequency": 1, "speed": 0, "direction": 90 });
    let Some(out) = Fx::new("waves", params).input(halves(RED, BLUE)).shot() else { return };
    out.check(&[((40, 8), RED), ((50, 8), BLUE), ((10, 8), RED)]);
}
