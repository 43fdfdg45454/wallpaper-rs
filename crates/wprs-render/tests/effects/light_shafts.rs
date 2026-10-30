use super::*;

fn gradient() -> RgbaImage {
    RgbaImage::from_fn(W, 1, |x, _| Rgba([255 - level(x), 0, level(x), 255]))
}

fn level(x: u32) -> u8 {
    (x * 255 / (W - 1)) as u8
}

#[test]
fn light_shafts_draw_beams_of_their_own_that_move_only_with_a_speed() {
    let still = |more: Value| {
        let mut params = json!({ "speed": 0, "intensity": 2, "feather": 0.2, "smoothness": 0.2, "color": "#ffffff" });
        params.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
        Fx::new("light_shafts", params).input(solid([0; 3])).image("ramp", gradient(), false)
    };
    let Some(linear) = still(json!({ "angle": 90 })).shot() else { return };
    let row: Vec<u8> = (0..W).map(|x| linear.at(x, 16)[0]).collect();
    let (lo, hi) = (row.iter().min().copied().unwrap_or(0), row.iter().max().copied().unwrap_or(0));
    assert!(hi > 120 && lo < hi / 2, "beams {lo}..{hi}");
    let mean = |y: u32| (0..W).map(|x| u32::from(linear.at(x, y)[0])).sum::<u32>() / W;
    assert!(mean(H - 1) * 4 < mean(16) && mean(H - 1) * 2 < mean(0), "{} {} {}", mean(0), mean(16), mean(H - 1));
    let Some(radial) = still(json!({ "direction": 1, "center": [0.5, 0.5] })).shot() else { return };
    let Some(corner) = still(json!({ "direction": 2, "center": [0, 0] })).shot() else { return };
    assert!(radial.diff(&linear) > 4.0 && corner.diff(&radial) > 4.0);
    let Some(colored) = still(json!({ "angle": 90, "gradient": "ramp" })).shot() else { return };
    assert!(colored.mean()[2] > 4.0 && colored.mean()[1] < 1.0, "{:?}", colored.mean());
    let Some(mut run) = Fx::new("light_shafts", json!({ "speed": 2, "intensity": 2 })).input(solid([0; 3])).run()
    else {
        return;
    };
    run.runtime.seek(0.0);
    let first = run.shot();
    run.runtime.seek(1.5);
    assert!(run.shot().diff(&first) > 1.0);
    Shot::sheet(&[linear, radial, corner, colored, first]).looks_like("light_shafts");
}
