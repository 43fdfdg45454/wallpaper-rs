use super::*;

#[test]
fn layers_compose_in_place() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "img", "type": "image", "image": "rb", "size": [100, 50] },
        { "id": "box", "type": "solid", "color": { "prop": "tone" }, "size": [20, 10], "transform": { "position": [10, 5], "origin": [0, 0] } },
        { "id": "dot", "type": "shape", "shape": { "ellipse": { "size": [10, 10] } }, "fill": "#ffffff", "anchor": "bottom_right", "transform": { "origin": [0.5, 0.5], "position": [-10, -10] } },
        { "id": "half", "type": "solid", "color": "#ffffff", "opacity": 0.5, "size": [10, 10], "transform": { "position": [80, 10] } }
    ]);
    let mut shot =
        Shot::new(r, canvas("#000000"), layers, &[("rb", halves([255, 0, 0, 255], [0, 0, 255, 255]))]);
    let pixels = shot.pixels(r);
    check(
        &pixels,
        &[
            ((2, 40), [255, 0, 0]),
            ((97, 40), [0, 0, 255]),
            ((15, 8), [0, 255, 0]),
            ((90, 40), [255, 255, 255]),
        ],
    );
    check(&pixels, &[((96, 46), [0, 0, 255]), ((80, 10), [188, 188, 255])]);
}

#[test]
fn blend_modes_and_blend_spaces() {
    let Some(r) = renderer() else { return };
    let modes = [
        ("normal", [255, 0, 0]),
        ("multiply", [128, 0, 0]),
        ("screen", [255, 128, 128]),
        ("add", [255, 128, 128]),
        ("difference", [229, 128, 128]),
        ("darken", [128, 0, 0]),
        ("luminosity", [188, 188, 188]),
        ("tint", [128, 0, 0]),
        ("diffuse_light", [175, 128, 128]),
        ("darker_color", [128, 128, 128]),
        ("lighter_color", [255, 0, 0]),
    ];
    for (mode, expected) in modes {
        let layers = json!([{ "id": "s", "type": "solid", "color": "#ff0000", "blend": mode, "size": [50, 50], "transform": { "position": [75, 25] } }]);
        let pixels = Shot::new(r, canvas("#808080"), layers, &[]).pixels(r);
        assert!(near(at(&pixels, 75, 25), expected, 4), "{mode}: {:?}", at(&pixels, 75, 25));
        assert!(near(at(&pixels, 25, 25), [128, 128, 128], 2), "{mode} backdrop");
    }
    let fade = |space: &str| {
        let layers = json!([{ "id": "s", "type": "solid", "color": "#ffffff", "opacity": 0.5 }]);
        let canvas = json!({ "size": [100, 50], "background": "#000000", "blend_space": space });
        at(&Shot::new(r, canvas, layers, &[]).pixels(r), 50, 25)
    };
    assert!(near(fade("linear"), [188, 188, 188], 2) && near(fade("srgb"), [128, 128, 128], 2));
}

#[test]
fn blend_modes_keep_light_above_white_and_whole_colors_ignore_opacity() {
    let Some(r) = renderer() else { return };
    let mixed = |first: Value, space: &str| {
        let half = json!({ "id": "half", "type": "solid", "color": "#808080", "blend": "multiply" });
        let canvas = json!({ "size": [100, 50], "background": "#ffffff", "blend_space": space });
        at(&Shot::new(r, canvas, json!([first, half]), &[]).pixels(r), 50, 25)
    };
    let lit = json!({ "id": "lit", "type": "solid", "color": "#ffffff", "blend": "diffuse_light" });
    let added = json!({ "id": "add", "type": "solid", "color": "#ffffff", "blend": "add", "opacity": 0.5 });
    let [lit, added] = [lit, added].map(|first| mixed(first, "linear")[0]);
    assert!(lit > 160 && added > 140, "light above white was clipped: {lit} {added}");
    assert!(
        mixed(json!({ "id": "none", "type": "solid", "color": "#000000", "opacity": 0.0 }), "linear")[0]
            <= 130
    );
    for (mode, expected) in [("darker_color", [255, 0, 0]), ("lighter_color", [128, 128, 128])] {
        let layers =
            json!([{ "id": "s", "type": "solid", "color": "#ff0000", "blend": mode, "opacity": 0.25 }]);
        let canvas = json!({ "size": [100, 50], "background": "#808080", "blend_space": "srgb" });
        let pixels = Shot::new(r, canvas, layers, &[]).pixels(r);
        assert!(near(at(&pixels, 50, 25), expected, 4), "{mode}: {:?}", at(&pixels, 50, 25));
    }
}

#[test]
fn masks_cut_layers() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "tex", "type": "solid", "color": "#ff0000", "size": [50, 50], "transform": { "position": [25, 25] }, "mask": "lr" },
        { "id": "round", "type": "solid", "color": "#00ff00", "size": [50, 50], "transform": { "position": [75, 25] }, "mask": { "shape": { "ellipse": { "size": [30, 30] } } } }
    ]);
    let pixels =
        Shot::new(r, canvas("#000000"), layers, &[("lr", halves([255; 4], [0, 0, 0, 255]))]).pixels(r);
    check(
        &pixels,
        &[((10, 25), [255, 0, 0]), ((40, 25), [0, 0, 0]), ((75, 25), [0, 255, 0]), ((53, 3), [0, 0, 0])],
    );
    for (visible, matte) in [(true, [0, 255, 0]), (false, [0, 0, 0])] {
        let layers = json!([
            { "id": "round", "type": "shape", "shape": { "ellipse": { "size": [30, 30] } }, "fill": "#00ff00",
              "transform": { "position": [75, 25] }, "visible": visible },
            { "id": "inverse", "type": "solid", "color": "#0000ff", "mask": { "layer": "round", "invert": true } }
        ]);
        let pixels = Shot::new(r, canvas("#000000"), layers, &[]).pixels(r);
        check(&pixels, &[((75, 25), matte), ((10, 25), [0, 0, 255]), ((53, 3), [0, 0, 255])]);
    }
}
