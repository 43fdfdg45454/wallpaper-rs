use super::*;

#[test]
fn water_rings_move_pixels_around_a_still_center() {
    let params = json!({ "amplitude": 2, "wavelength": 16, "speed": 6 });
    let Some(mut run) = Fx::new("water_rings", params).input(ramp()).run() else { return };
    let first = run.shot();
    let input = ramp();
    let near = |x: u32, y: u32| {
        first.at(x, y).iter().zip(&input.get_pixel(x, y).0).all(|(a, b)| a.abs_diff(*b) <= 3)
    };
    assert!(near(32, 16) && !(0..W).all(|x| near(x, 4)));
    run.runtime.advance(0.5);
    assert!(run.shot().diff(&first) > 0.5);
    let Some(flat) = Fx::new("water_rings", json!({ "amplitude": 0 })).input(ramp()).shot() else { return };
    assert!(flat.diff(&Shot(ramp())) < 0.5);
}
