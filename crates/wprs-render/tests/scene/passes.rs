use super::*;

#[test]
fn effects_render_their_passes() {
    let Some(r) = renderer() else { return };
    let rb = || halves([255, 0, 0, 255], [0, 0, 255, 255]);
    let gray = json!([{ "id": "s", "type": "solid", "color": "#ff0000", "effects": [{ "effect": "color_adjust", "params": { "saturation": 0 } }] }]);
    check(&Shot::new(r, canvas("#000000"), gray, &[]).pixels(r), &[((50, 25), [127, 127, 127])]);
    let blur = json!([{ "id": "i", "type": "image", "image": "rb", "size": [100, 50], "effects": [{ "effect": "blur", "params": { "radius": 12 } }] }]);
    let pixels = Shot::new(r, canvas("#000000"), blur, &[("rb", rb())]).pixels(r);
    let [red, _, blue] = at(&pixels, 50, 25);
    assert!(red > 60 && blue > 60 && red < 250 && blue < 250, "blurred edge {:?}", at(&pixels, 50, 25));
    check(&pixels, &[((25, 25), [255, 0, 0]), ((75, 25), [0, 0, 255])]);
    let waves = json!([{ "id": "i", "type": "image", "image": "rb", "size": [100, 50],
        "effects": [{ "effect": "waves", "params": { "amplitude": 10, "frequency": 1, "speed": 0, "direction": 90 } }] }]);
    let pixels = Shot::new(r, canvas("#000000"), waves, &[("rb", rb())]).pixels(r);
    check(&pixels, &[((55, 12), [255, 0, 0]), ((45, 37), [0, 0, 255]), ((25, 25), [255, 0, 0])]);
    let glow = json!([{ "id": "s", "type": "solid", "color": "#ffffff", "size": [10, 10],
        "effects": [{ "effect": "bloom", "params": { "threshold": 0.5, "intensity": 2, "radius": 8 } }] }]);
    let pixels = Shot::new(r, canvas("#000000"), glow, &[]).pixels(r);
    assert!(at(&pixels, 58, 25)[0] > 20 && at(&pixels, 50, 25) == [255; 3], "{:?}", at(&pixels, 58, 25));
    check(&pixels, &[((5, 5), [0, 0, 0])]);
}

#[test]
fn groups_isolate_and_spectra_follow_audio() {
    let Some(r) = renderer() else { return };
    let layers = json!([{ "id": "g", "type": "group", "opacity": 0.5, "layers": [
        { "id": "a", "type": "solid", "color": "#ff0000", "size": [60, 50], "transform": { "position": [30, 25] } },
        { "id": "b", "type": "solid", "color": "#0000ff", "size": [60, 50], "transform": { "position": [70, 25] } }
    ] }]);
    check(
        &Shot::new(r, canvas("#000000"), layers, &[]).pixels(r),
        &[((10, 25), [188, 0, 0]), ((50, 25), [0, 0, 188]), ((90, 25), [0, 0, 188])],
    );
    let bars = json!([{ "id": "s", "type": "spectrum", "size": [100, 50], "bands": 16, "gap": 0.0, "color": "#ffffff" }]);
    let mut shot = Shot::new(r, canvas("#000000"), bars, &[]);
    check(&shot.pixels(r), &[((3, 25), [0, 0, 0])]);
    let mut audio = AudioFrame { silent: false, ..AudioFrame::default() };
    audio.average[..4].fill(1.0);
    audio.average[4..8].fill(0.5);
    assert!(shot.runtime.set_audio(audio));
    check(
        &shot.pixels(r),
        &[
            ((3, 25), [255, 255, 255]),
            ((9, 12), [0, 0, 0]),
            ((9, 40), [255, 255, 255]),
            ((60, 40), [0, 0, 0]),
        ],
    );
}

#[test]
fn static_parts_are_cached_and_steady_frames_create_nothing() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "img", "type": "image", "image": "rb", "size": [100, 50], "effects": [{ "effect": "blur", "params": { "radius": 4 } }] },
        { "id": "spin", "type": "solid", "color": "#ffffff", "size": [10, 10], "transform": { "rotation": { "keys": [[0, 0], [10, 360]], "loop": "repeat" } } },
        { "id": "text", "type": "text", "text": "Hi", "size": 30, "color": "#ffffff", "transform": { "position": [20, 25] } },
        { "id": "pulse", "type": "solid", "color": "#00ff00", "size": [20, 20], "effects": [{ "effect": "blur", "params": { "radius": 2 } }],
          "transform": { "position": [80, 25], "scale": { "keys": [[0, 1], [10, 1.4]], "loop": "pingpong" } } }
    ]);
    let mut shot =
        Shot::new(r, canvas("#000000"), layers, &[("rb", halves([255, 0, 0, 255], [0, 0, 255, 255]))]);
    let first = shot.pixels(r);
    assert!(shot.view.stats().created > 0);
    shot.runtime.seek(1.25);
    let second = shot.pixels(r);
    let stats = shot.view.stats();
    assert_eq!((stats.created, stats.reused), (0, 1), "{stats:?}");
    assert!(stats.passes >= 1 && stats.draws >= 2, "{stats:?}");
    assert_ne!(first, second);
    let text = (0..50)
        .flat_map(|x| (10..40).map(move |y| (x, y)))
        .filter(|&(x, y)| at(&second, x, y)[1] > 200)
        .count();
    let fonts = fontdb_has_fonts();
    assert!(!fonts || text > 10, "text pixels {text}");
    for t in [2.5, 5.0, 7.5] {
        shot.runtime.seek(t);
        shot.pixels(r);
        assert_eq!(shot.view.stats().created, 0, "growing layer at {t} s");
    }
}

#[test]
fn a_view_swaps_only_a_changed_image() {
    let Some(r) = renderer() else { return };
    let layers = json!([{ "id": "img", "type": "image", "image": "photo", "size": [100, 50] }]);
    let (red, blue) =
        (halves([255, 0, 0, 255], [255, 0, 0, 255]), halves([0, 0, 255, 255], [0, 0, 255, 255]));
    let mut shot = Shot::new(r, canvas("#000000"), layers, &[("photo", red.clone())]);
    check(&shot.pixels(r), &[((50, 25), [255, 0, 0])]);
    let scene = shot.runtime.scene.clone();
    let assets = |image| SceneAssets { images: [("photo".to_string(), image)].into(), ..Default::default() };
    let mut swap =
        |image| r.refresh_scene(&mut shot.view, (&scene, shot.runtime.props()), &assets(image)).unwrap();
    assert!(!swap(red));
    assert!(swap(blue));
    check(&shot.pixels(r), &[((50, 25), [0, 0, 255])]);
}
