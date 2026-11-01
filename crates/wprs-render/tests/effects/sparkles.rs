use super::*;
use wprs_scene::{Quality, Wake};

#[test]
fn glitter_sparkles_in_steps_and_sleeps_between_them() {
    let Some(mut run) = Fx::new("glitter", json!({ "density": 0.6, "speed": 2 })).input(solid([40; 3])).run() else {
        return;
    };
    let first = run.shot();
    assert!(first.mean()[0] > 42.0, "{:?}", first.mean());
    assert!(matches!(run.runtime.wake(), Wake::At(_)), "{:?}", run.runtime.wake());
    run.runtime.seek(0.6);
    let later = run.shot();
    assert!(later.diff(&first) > 0.5);
    let Some(coarse) = Fx::new("glitter", json!({ "density": 0.6 })).input(solid([40; 3])).quality(Quality::Q0).shot()
    else {
        return;
    };
    assert!(coarse.diff(&first) > 0.2);
    let Some(mut still) = Fx::new("glitter", json!({ "speed": 0 })).input(solid([40; 3])).run() else { return };
    still.shot();
    assert_eq!(still.runtime.wake(), Wake::Idle);
    Shot::sheet(&[first, later, coarse]).looks_like("glitter");
}

#[test]
fn moving_patterns_draw_only_while_they_have_a_speed() {
    let cases = [
        ("caustics", json!({ "speed": 1, "strength": 2 }), json!({ "speed": 0, "strength": 2 })),
        ("electric", json!({ "speed": 2, "bounds": [0.2, 0.5] }), json!({ "speed": 0, "bounds": [0.2, 0.5] })),
        ("fire", json!({ "speed": 2, "strength": 6 }), json!({ "speed": 0, "strength": 6 })),
        ("vhs", json!({ "speed": 2, "strength": 1 }), json!({ "speed": 0, "strength": 1 })),
        ("pulse", json!({ "speed": 2, "bounds": [0.2, 1] }), json!({ "speed": 0, "bounds": [0.2, 0.5] })),
        ("spin", json!({ "speed": 90 }), json!({ "angle": 30 })),
        ("displace", json!({ "scroll": [0.5, 0] }), json!({})),
    ];
    let mut sheet = Vec::new();
    for (effect, moving, still) in cases {
        let Some(mut run) = Fx::new(effect, moving).input(squares()).run() else { return };
        let first = run.shot();
        let stepped = effect == "electric" && matches!(run.runtime.wake(), Wake::At(_));
        assert!(stepped || run.runtime.wake() == Wake::Frame, "{effect}");
        run.runtime.seek(0.7);
        let later = run.shot();
        assert!(later.diff(&first) > 0.3, "{effect} did not move: {}", later.diff(&first));
        let Some(mut rest) = Fx::new(effect, still).input(squares()).run() else { return };
        let shown = rest.shot();
        assert_eq!(rest.runtime.wake(), Wake::Idle, "{effect} keeps drawing without a speed");
        assert!(shown.diff(&Shot(squares())) > 0.3, "{effect} drew nothing");
        rest.runtime.seek(2.0);
        assert_eq!(rest.shot().diff(&shown), 0.0, "{effect} changed without a speed");
        sheet.extend([first, later]);
    }
    Shot::sheet(&sheet).looks_like("patterns");
}
