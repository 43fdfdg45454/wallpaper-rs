use super::*;

#[test]
fn reflection_mirrors_the_top_below_the_line_and_fades() {
    let sky =
        RgbaImage::from_fn(W, H, |_, y| Rgba(if y < H / 2 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    let still = json!({ "line": 0.5, "opacity": 1, "fade": 0, "amplitude": 0 });
    let Some(out) = Fx::new("reflection", still).input(sky.clone()).shot() else { return };
    out.check(&[((10, 8), RED), ((10, 20), RED), ((50, 30), RED)]);
    let faded = json!({ "line": 0.5, "opacity": 1, "fade": 1, "amplitude": 0 });
    let Some(out) = Fx::new("reflection", faded).input(sky).shot() else { return };
    let red = [17, 24, 30].map(|y| out.at(10, y)[0]);
    assert!(red[0] > 200 && red[0] > red[1] && red[1] > red[2] && out.at(10, 30)[2] > 200, "{red:?}");
}
