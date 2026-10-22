use super::*;

fn cube(f: impl Fn(u8) -> u8) -> String {
    let rows: String =
        (0..8u8).map(|i| format!("{} {} {}\n", f(i & 1), f((i >> 1) & 1), f(i >> 2))).collect();
    format!("LUT_3D_SIZE 2\n{rows}")
}

#[test]
fn color_lut_grades_by_its_intensity() {
    let fx = |f: fn(u8) -> u8, intensity: f32, input: RgbaImage| {
        let params = json!({ "lut": "grade", "intensity": intensity });
        Fx::new("color_lut", params).input(input).lut("grade", &cube(f)).shot()
    };
    let Some(inverted) = fx(|v| 1 - v, 1.0, solid(RED)) else { return };
    inverted.check(&[((32, 16), [0, 255, 255])]);
    let Some(half) = fx(|v| 1 - v, 0.5, solid([0; 3])) else { return };
    half.check(&[((32, 16), [188; 3])]);
    let Some(same) = fx(|v| v, 1.0, ramp()) else { return };
    let Some(plain) = Fx::chain(json!([])).shot() else { return };
    assert!(same.diff(&plain) < 1.0, "identity LUT moved {}", same.diff(&plain));
    let Some(missing) = Fx::new("color_lut", json!({})).input(solid(RED)).shot() else { return };
    missing.check(&[((32, 16), RED)]);
}
