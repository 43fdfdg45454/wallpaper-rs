use super::fonts::boxes;
use super::*;

fn styled(r: &Renderer, more: Value) -> Shot {
    let mut t = json!({ "id": "t", "type": "text", "text": "A", "font": "b", "size": 20, "color": "#ffffff",
        "transform": { "position": [50, 25] } });
    t.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    let scene = json!({ "canvas": canvas("#000000"), "assets": { "b": { "path": "b.ttf" } }, "layers": [t] });
    let mut shot = Shot::with(r, scene, &[]);
    let assets = SceneAssets { fonts: [("b".to_string(), Arc::new(boxes()))].into(), ..Default::default() };
    shot.view = r.scene_view(&shot.runtime.scene, &assets, shot.runtime.props()).unwrap();
    shot
}

fn count(pixels: &[u8], test: impl Fn([u8; 3]) -> bool) -> usize {
    (0..100).flat_map(|x| (0..50).map(move |y| (x, y))).filter(|&(x, y)| test(at(pixels, x, y))).count()
}

fn span(pixels: &[u8]) -> u32 {
    let lit: Vec<u32> = (0..100).filter(|&x| (0..50).any(|y| at(pixels, x, y)[0] > 128)).collect();
    lit.last().unwrap() - lit.first().unwrap()
}

#[test]
fn texts_draw_outlines_shadows_blurs_and_spaced_letters() {
    let Some(r) = renderer() else { return };
    let plain = styled(r, json!({})).pixels(r);
    let white = count(&plain, |p| p[0] > 200 && p[2] > 200);
    assert!(white > 60, "{white}");
    let wide = styled(r, json!({ "text": "AA", "letter_spacing": 12 })).pixels(r);
    let tight = styled(r, json!({ "text": "AA" })).pixels(r);
    assert!(span(&wide) >= span(&tight) + 10, "{} {}", span(&wide), span(&tight));
    let ringed = styled(r, json!({ "outline": { "color": "#ff0000", "width": 3 } })).pixels(r);
    let red = count(&ringed, |p| p[0] > 200 && p[1] < 60);
    assert!(red > 30 && count(&ringed, |p| p[0] > 200 && p[1] > 200).abs_diff(white) < 12, "{red}");
    let cast = json!({ "shadow": { "color": "#0000ff", "offset": [8, 8], "blur": 0 } });
    let shaded = styled(r, cast).pixels(r);
    assert!(count(&shaded, |p| p[2] > 200 && p[0] < 60) > 20, "the shadow shows beside the letter");
    let soft = styled(r, json!({ "blur": 4 })).pixels(r);
    assert!(count(&soft, |p| p[0] > 20 && p[0] < 200) > count(&plain, |p| p[0] > 20 && p[0] < 200) + 20);
    let both = json!({ "outline": { "color": "#ff0000", "width": 2 }, "shadow": { "color": "#00ff00", "offset": [4, 3], "blur": 2 },
        "text": "AxA", "render": "sdf" });
    looks_like("styles", &styled(r, both).pixels(r));
}

#[test]
fn distance_fields_scale_without_drawing_the_letters_again() {
    let Some(r) = renderer() else { return };
    let grow = json!({ "render": "sdf", "transform": { "position": [50, 25], "scale": { "keys": [[0, 1], [1, 2]] } } });
    let mut shot = styled(r, grow);
    let small = shot.pixels(r);
    shot.runtime.seek(1.0);
    let large = shot.pixels(r);
    assert!(count(&large, |p| p[0] > 128) > 3 * count(&small, |p| p[0] > 128));
    shot.runtime.seek(0.5);
    shot.pixels(r);
    let raster = json!({ "transform": { "position": [50, 25], "scale": { "keys": [[0, 1], [1, 2]] } } });
    let mut plain = styled(r, raster);
    for t in [0.0, 1.0, 0.5] {
        plain.runtime.seek(t);
        plain.pixels(r);
    }
    let made = |s: &Shot| s.view.stats().created;
    assert_eq!(made(&plain), made(&shot) + 2, "plain text is drawn again at its new size, the field is not");
    let blurred = json!({ "shadow": { "color": "#0000ff", "offset": [0, 0], "blur": 16 } });
    let mut low = styled(r, blurred);
    let soft = |p: &[u8]| count(p, |c| c[2] > 8 && c[0] < 8);
    let (high, lowest) = (soft(&low.pixels(r)), soft(&crate::stage::at_level(&mut low, r, Quality::Q0)));
    assert!(lowest < high, "the lowest level narrows the shadow: {lowest} against {high}");
}
