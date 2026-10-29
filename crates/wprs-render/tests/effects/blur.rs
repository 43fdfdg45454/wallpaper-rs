use super::*;
use wprs_scene::Quality;

#[test]
fn blur_mixes_colors_across_an_edge() {
    let Some(out) = Fx::new("blur", json!({ "radius": 12 })).input(halves(RED, BLUE)).shot() else { return };
    let [red, _, blue] = out.at(32, 16);
    assert!(red > 60 && blue > 60 && red < 250 && blue < 250, "blurred edge {:?}", out.at(32, 16));
    out.check(&[((4, 16), RED), ((60, 16), BLUE)]);
    let lowest = Fx::new("blur", json!({ "radius": 12 })).input(halves(RED, BLUE)).quality(Quality::Q0);
    let Some(sharp) = lowest.shot() else { return };
    sharp.check(&[((31, 16), RED), ((32, 16), BLUE)]);
}

#[test]
fn blurs_and_warps_keep_the_color_of_every_screen_edge() {
    let effects = [
        ("blur", json!({ "radius": 12 })),
        ("waves", json!({ "amplitude": 8, "speed": 0 })),
        ("blur_motion", json!({ "angle": 45, "length": 24 })),
        ("twirl", json!({ "angle": 90, "radius": 40 })),
    ];
    for (effect, params) in effects {
        let Some(mut run) = Fx::new(effect, params).input(solid(RED)).run() else { return };
        let out = run.shot_at((50, 30));
        let edges = [(0, 0), (49, 0), (0, 29), (49, 29), (25, 29), (49, 15), (25, 0), (0, 15)];
        for (x, y) in edges {
            let [r, g, b] = out.at(x, y);
            assert!(r > 250 && g < 5 && b < 5, "{effect} darkened {x},{y} to {:?}", [r, g, b]);
        }
    }
}
