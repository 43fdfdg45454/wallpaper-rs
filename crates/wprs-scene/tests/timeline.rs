use wprs_scene::{Color, Easing, Repeat, Scene, Timeline, Value};

fn ramp(easing: Easing, repeat: Repeat) -> Timeline {
    let keyframes = vec![(1.0, Value::Number(0.0)), (3.0, Value::Number(1.0))];
    Timeline { target: "/layers/0/opacity".into(), keyframes, easing, repeat }
}

fn number(timeline: &Timeline, t: f32) -> f32 {
    match timeline.value_at(t) {
        Some(Value::Number(n)) => n,
        other => panic!("{other:?}"),
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn easing_curves() {
    let cases = [
        (Easing::Linear, 0.25, 0.5),
        (Easing::EaseIn, 0.0625, 0.25),
        (Easing::EaseOut, 0.4375, 0.75),
        (Easing::EaseInOut, 0.15625, 0.5),
        (Easing::Step, 0.0, 0.0),
    ];
    for (easing, quarter, half) in cases {
        let tl = ramp(easing, Repeat::Once);
        assert!(close(number(&tl, 1.5), quarter) && close(number(&tl, 2.0), half), "{easing:?}");
        assert_eq!(
            (number(&tl, 0.0), number(&tl, 1.0), number(&tl, 3.0), number(&tl, 9.0)),
            (0.0, 0.0, 1.0, 1.0)
        );
    }
}

#[test]
fn repeat_modes() {
    let looped = ramp(Easing::Linear, Repeat::Loop);
    for (t, v) in [(0.0, 0.5), (1.0, 0.0), (2.0, 0.5), (3.0, 0.0), (4.5, 0.75), (101.0, 0.0)] {
        assert!(close(number(&looped, t), v), "loop at {t}");
    }
    let ping = ramp(Easing::Linear, Repeat::PingPong);
    for (t, v) in [(0.0, 0.5), (2.0, 0.5), (3.0, 1.0), (4.0, 0.5), (5.0, 0.0), (6.0, 0.5), (7.5, 0.75)] {
        assert!(close(number(&ping, t), v), "ping-pong at {t}");
    }
}

#[test]
fn colors_interpolate_and_discrete_values_step() {
    let (black, orange) = (Color([0.0, 0.0, 0.0, 1.0]), Color([1.0, 0.5, 0.0, 1.0]));
    let colors = vec![(0.0, Value::Color(black)), (2.0, Value::Color(orange))];
    let tl = Timeline {
        target: "/layers/0/kind/color".into(),
        keyframes: colors,
        ..ramp(Easing::Linear, Repeat::Once)
    };
    assert_eq!(tl.value_at(1.0), Some(Value::Color(Color([0.5, 0.25, 0.0, 1.0]))));
    let flags = vec![(0.0, Value::Bool(false)), (1.0, Value::Bool(true))];
    let blink = Timeline { keyframes: flags, ..ramp(Easing::Linear, Repeat::Loop) };
    assert_eq!(
        (blink.value_at(0.9), blink.value_at(1.0)),
        (Some(Value::Bool(false)), Some(Value::Bool(false)))
    );
    assert_eq!(Timeline { keyframes: vec![], ..ramp(Easing::Linear, Repeat::Once) }.value_at(1.0), None);
    let single =
        Timeline { keyframes: vec![(5.0, Value::Number(0.7))], ..ramp(Easing::Linear, Repeat::Loop) };
    assert_eq!(
        (single.value_at(0.0), single.value_at(50.0)),
        (Some(Value::Number(0.7)), Some(Value::Number(0.7)))
    );
}

#[test]
fn scene_is_evaluated_at_time() {
    let scene = Scene::from_json(include_str!("sample.json")).unwrap();
    let at = |t| scene.at(t).unwrap();
    assert!(close(at(1.0).layers[3].opacity, 0.35) && close(at(3.0).layers[3].opacity, 0.35));
    assert!(close(at(0.0).layers[4].transform.y, 240.0) && close(at(60.0).layers[4].transform.y, 200.0));
    let Some(Value::Color(Color(c))) = at(12.5).get("/layers/3/kind/color") else { panic!() };
    assert!(c.iter().zip([0.55, 0.3, 0.65, 1.0]).all(|(a, b)| close(*a, b)), "{c:?}");
    assert_eq!(at(0.0).layers[0], scene.layers[0]);
}
