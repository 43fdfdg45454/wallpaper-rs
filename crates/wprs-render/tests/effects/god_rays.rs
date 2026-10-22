use super::*;

#[test]
fn god_rays_reach_out_from_the_source() {
    let lit = |x: u32, y: u32| x.abs_diff(32) < 3 && y.abs_diff(5) < 3;
    let sun = RgbaImage::from_fn(W, H, |x, y| Rgba(if lit(x, y) { [255; 4] } else { [0, 0, 0, 255] }));
    let params = json!({ "source": [0.5, 0.16], "threshold": 0.5, "exposure": 2 });
    let Some(out) = Fx::new("god_rays", params).input(sun).shot() else { return };
    assert!(out.at(32, 16)[0] > 30, "ray {:?}", out.at(32, 16));
    out.check(&[((32, 5), [255; 3]), ((4, 28), [0; 3]), ((62, 5), [0; 3])]);
}

#[test]
fn god_rays_on_an_adjustment_layer_start_at_their_canvas_source() {
    let lit = |x: u32, y: u32| x.abs_diff(16) < 3 && y.abs_diff(8) < 3;
    let sun = RgbaImage::from_fn(W, H, |x, y| Rgba(if lit(x, y) { [255; 4] } else { [0, 0, 0, 255] }));
    let params = json!({ "source": [0.25, 0.25], "threshold": 0.5, "exposure": 2 });
    let fx = || Fx::new("god_rays", params.clone()).input(sun.clone()).adjust().run();
    let (Some(mut full), Some(mut crop)) = (fx(), fx()) else { return };
    let (full, crop) = (full.shot(), crop.shot_at((48, H)));
    assert!(full.at(16, 20)[0] > 30, "ray {:?}", full.at(16, 20));
    crop.check(&(4..44).flat_map(|x| [4, 16, 28].map(|y| ((x, y), full.at(x + 8, y)))).collect::<Vec<_>>());
}
