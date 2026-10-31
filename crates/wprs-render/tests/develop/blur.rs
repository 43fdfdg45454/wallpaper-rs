use super::*;

#[test]
fn blur_softens_edges_and_keeps_flat_areas() {
    let Some(r) = renderer() else { return };
    let image = RgbaImage::from_fn(64, 32, |x, _| Rgba(if x < 32 { [0, 0, 0, 255] } else { [255; 4] }));
    let blur = op(json!({"type": "blur", "radius": 100}));
    let out = develop(r, &image, &blur, (64, 32));
    let edge = at(&out, 31, 16)[0];
    assert!((20..235).contains(&edge), "edge {edge}");
    expect(at(&out, 1, 16), [0; 3]);
    expect(at(&out, 62, 16), [255; 3]);
    let flat = develop(r, &flat([90, 160, 30]), &op(json!({"type": "blur", "radius": 200})), (64, 64));
    expect(at(&flat, 32, 32), [90, 160, 30]);
    let value = json!({"ops": [{"id": "b", "type": "blur", "radius": 100, "mask": "left"}, {"id": "e", "type": "exposure", "ev": 0}],
        "masks": {"left": {"type": "linear", "from": [0.4, 0.5], "to": [0.3, 0.5]}}});
    let masked = develop(r, &image, &edit(value), (64, 32));
    expect(at(&masked, 33, 16), [255; 3]);
}

fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let poly =
        t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    (1.0 - poly * (-x * x).exp()).copysign(x)
}

#[test]
fn blur_spreads_an_edge_like_a_gaussian_with_its_radius_as_deviation() {
    let Some(r) = renderer() else { return };
    for (radius, width) in [(10.0, 1000), (25.0, 1000), (50.0, 500), (100.0, 1000)] {
        let image =
            RgbaImage::from_fn(width, 64, |x, _| Rgba(if x < width / 2 { [0, 0, 0, 255] } else { [255; 4] }));
        let out = develop(r, &image, &op(json!({"type": "blur", "radius": radius})), (width, 64));
        let sigma = radius * f64::from(width) / 1000.0;
        let reference =
            |x: u32| 0.5 + 0.5 * erf((f64::from(x) + 0.5 - f64::from(width / 2)) / (sigma * 2f64.sqrt()));
        let worst = (0..width).map(|x| (linear(at(&out, x, 32)[0]) - reference(x)).abs()).fold(0.0, f64::max);
        assert!(worst < 0.06, "radius {radius} at {width} pixels is {worst} away from the reference");
    }
}
