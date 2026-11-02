use super::*;

fn strip(r: &Renderer, rgb: Value, on: bool) -> Option<(u32, u32, Vec<u8>)> {
    let layers = json!([{ "id": "p", "type": "image", "image": "p" }, rgb]);
    let text =
        json!({ "canvas": canvas("#000000"), "assets": { "p": { "path": "p.png" } }, "layers": layers });
    let mut shot = Shot::with(r, text, &[("p", halves([255, 0, 0, 255], [0, 0, 255, 255]))]);
    shot.pixels(r);
    shot.view.want_rgb(on);
    shot.pixels(r);
    (0..200).find_map(|_| {
        std::thread::sleep(std::time::Duration::from_millis(5));
        shot.view.rgb(r)
    })
}

#[test]
fn an_rgb_layer_sends_its_capture_shrunk_to_the_strip() {
    let Some(r) = renderer() else { return };
    let whole = strip(r, json!({ "id": "rgb", "type": "adjustment", "rgb": true }), true).unwrap();
    assert_eq!((whole.0, whole.1, whole.2.len()), (32, 16, 32 * 16 * 3));
    let at = |x: usize, y: usize| &whole.2[(y * 32 + x) * 3..][..3];
    assert_eq!((at(2, 8), at(29, 8)), (&[255, 0, 0][..], &[0, 0, 255][..]));
    let region = json!({ "id": "rgb", "type": "adjustment", "rgb": true, "size": [40, 10], "transform": { "position": [70, 25] } });
    let (w, h, rgb) = strip(r, region, true).unwrap();
    assert_eq!((w, h), (40, 10));
    assert!(rgb.chunks(3).all(|c| c == [0, 0, 255]), "{rgb:?}");
    let wide = json!({ "id": "rgb", "type": "adjustment", "rgb": true, "size": [180, 20], "transform": { "position": [50, 25] } });
    assert_eq!(strip(r, wide, true).map(|(w, h, _)| (w, h)), Some((64, 7)));
    assert!(strip(r, json!({ "id": "rgb", "type": "adjustment", "rgb": true }), false).is_none());
    assert!(strip(r, json!({ "id": "rgb", "type": "adjustment" }), true).is_none());
}

#[test]
fn the_rgb_layer_leaves_the_picture_alone() {
    let Some(r) = renderer() else { return };
    let layers = |rgb: bool| json!([{ "id": "p", "type": "image", "image": "p" }, { "id": "rgb", "type": "adjustment", "rgb": rgb }]);
    let shot = |rgb: bool| {
        let text = json!({ "canvas": canvas("#000000"), "assets": { "p": { "path": "p.png" } }, "layers": layers(rgb) });
        let mut shot = Shot::with(r, text, &[("p", halves([255, 0, 0, 255], [0, 0, 255, 255]))]);
        shot.view.want_rgb(true);
        shot.pixels(r)
    };
    assert_eq!(shot(true), shot(false));
    let scene = |rgb: bool| {
        Scene::load(
            &json!({ "schema": SCHEMA, "canvas": canvas("#000000"), "assets": { "p": { "path": "p.png" } },
                "layers": layers(rgb) })
            .to_string(),
            &BTreeMap::new(),
        )
    };
    assert!(wprs_render::lit(&scene(true).unwrap()) && !wprs_render::lit(&scene(false).unwrap()));
}
