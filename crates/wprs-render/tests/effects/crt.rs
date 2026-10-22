use super::*;

fn only(key: &str, value: f32) -> Value {
    let mut params =
        json!({ "curvature": 0, "scan": 0, "phosphor": 0, "vignette": 0, "glow": 0, "pitch": 4 });
    params[key] = json!(value);
    params
}

#[test]
fn crt_draws_scanlines_phosphors_and_curved_corners() {
    let fx = |params: Value| Fx::new("crt", params).input(solid([255; 3])).shot();
    let Some(lines) = fx(only("scan", 0.6)) else { return };
    let (bright, dark) = (lines.at(32, 1)[0], lines.at(32, 3)[0]);
    assert!(bright > 240 && dark + 15 < bright && dark > 200, "{bright} {dark}");
    let Some(curved) = fx(only("curvature", 1.0)) else { return };
    curved.check(&[((0, 0), [0; 3]), ((32, 16), [255; 3])]);
    let Some(mask) = fx(only("phosphor", 1.0)) else { return };
    mask.check(&[((30, 16), [255, 0, 0]), ((31, 16), [0, 255, 0]), ((32, 16), [0, 0, 255])]);
}
