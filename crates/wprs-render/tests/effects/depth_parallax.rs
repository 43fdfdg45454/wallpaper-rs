use super::*;

fn relief(mut params: Value, depth: &RgbaImage) -> Fx {
    params["depth"] = json!("depth");
    Fx::new("depth_parallax", params).image("depth", depth.clone(), true)
}

fn moved(fx: impl Fn() -> Fx) -> Option<(Shot, Shot)> {
    let (still, mut run) = (fx().shot()?, fx().run()?);
    run.pointer([1.0, 0.5]);
    Some((still, run.shot()))
}

#[test]
fn depth_parallax_slides_near_parts_over_far_ones() {
    let depth = halves([255; 3], [0; 3]);
    let fx = || relief(json!({ "strength": 8, "focus": 0 }), &depth).input(halves(RED, BLUE));
    let Some((still, near)) = moved(fx) else { return };
    still.check(&[((29, 16), RED), ((35, 16), BLUE), ((60, 16), BLUE)]);
    near.check(&[((29, 16), RED), ((35, 16), RED), ((38, 16), RED), ((60, 16), BLUE)]);
}

#[test]
fn depth_parallax_keeps_the_focus_depth_still() {
    let depth = halves([255; 3], [128; 3]);
    let Some((still, near)) = moved(|| relief(json!({ "strength": 8, "focus": 0.5 }), &depth)) else { return };
    near.check(&[((48, 16), still.at(48, 16)), ((60, 16), still.at(60, 16))]);
    assert!(near.at(16, 16)[0].abs_diff(still.at(16, 16)[0]) > 12, "{:?}", near.at(16, 16));
}

#[test]
fn depth_parallax_takes_fewer_steps_at_lower_quality() {
    let stripes = RgbaImage::from_fn(W, H, |x, _| Rgba(if x % 8 < 2 { [255; 4] } else { [0, 0, 0, 255] }));
    let shot = |steps: u32, quality| {
        let fx = || relief(json!({ "strength": 32, "focus": 0, "steps": steps }), &stripes).quality(quality);
        Some(moved(fx)?.1)
    };
    let (Some(fine), Some(coarse), Some(low)) = (shot(16, Quality::Q3), shot(8, Quality::Q3), shot(16, Quality::Q1))
    else {
        return;
    };
    assert!(fine.diff(&coarse) > 5.0, "{}", fine.diff(&coarse));
    assert_eq!(low.diff(&coarse), 0.0);
}

#[test]
fn depth_parallax_on_an_adjustment_layer_keeps_its_depth_on_the_canvas() {
    let depth = RgbaImage::from_fn(W, H, |x, _| Rgba(if x < 16 { [255; 4] } else { [0, 0, 0, 255] }));
    let fx = || relief(json!({ "strength": 8, "focus": 0 }), &depth).adjust().run();
    let (Some(mut full), Some(mut crop)) = (fx(), fx()) else { return };
    full.pointer([1.0, 0.5]);
    crop.pointer([1.0, 0.5]);
    let (full, crop) = (full.shot(), crop.shot_at((48, H)));
    crop.check(&(10..46).flat_map(|x| [4, 16, 28].map(|y| ((x, y), full.at(x + 8, y)))).collect::<Vec<_>>());
}
