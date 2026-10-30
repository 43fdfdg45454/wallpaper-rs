use super::*;

#[test]
fn tint_mixes_its_color_with_a_blend_mode_or_keeps_the_brightness() {
    let red = json!({ "color": "#ff0000", "blend": "multiply" });
    let Some(multiply) = Fx::new("tint", red).input(solid([255; 3])).shot() else { return };
    multiply.check(&[((10, 10), [255, 0, 0]), ((50, 20), [255, 0, 0])]);
    let screen = json!({ "color": "#0000ff", "blend": "screen", "amount": 0.5 });
    let Some(half) = Fx::new("tint", screen).input(solid([0; 3])).shot() else { return };
    let [r, _, b] = half.at(32, 16);
    assert!(r < 4 && b > 150 && b < 230, "{:?}", half.at(32, 16));
    let colorize = json!({ "color": "#0000ff", "mode": 1 });
    let Some(colored) = Fx::new("tint", colorize).shot() else { return };
    let [r, g, b] = colored.at(40, 16);
    assert!(b > r + 40 && b > g + 40, "{:?}", colored.at(40, 16));
    assert!(colored.at(0, 16) == [0; 3] && colored.at(63, 16)[2] > 240, "{:?}", colored.at(63, 16));
    let round = json!([{ "effect": "tint", "params": { "color": "#00ff00", "blend": "normal" },
        "mask": { "shape": { "ellipse": { "size": [32, 24] } } } }]);
    let Some(masked) = Fx::chain(round).input(solid([255; 3])).shot() else { return };
    masked.check(&[((32, 16), [0, 255, 0]), ((2, 2), [255; 3]), ((61, 29), [255; 3])]);
    let toned = json!({ "color": "#ffaa00", "blend": "overlay", "amount": 0.75 });
    let Some(overlay) = Fx::new("tint", toned).shot() else { return };
    Shot::sheet(&[multiply, half, colored, masked, overlay]).looks_like("tint");
}
