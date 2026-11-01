use super::*;

fn green(s: &Shot, x: u32, y: u32) -> bool {
    let [r, g, b] = s.at(x, y);
    g > 200 && r < 40 && b < 40
}

fn phase() -> RgbaImage {
    halves([0; 3], [128; 3])
}

#[test]
fn moved_layers_show_what_lies_behind_with_background() {
    let iris = |on: bool| {
        let fx = Fx::new("iris", json!({ "follow": 1, "range": 12, "background": on }));
        let mut run = fx.input(solid([200; 3])).background("#00ff00").run()?;
        run.pointer([1.0, 0.5]);
        Some(run.shot())
    };
    let (Some(shown), Some(kept)) = (iris(true), iris(false)) else { return };
    assert!(green(&shown, 1, 16) && !green(&kept, 1, 16) && !green(&shown, 40, 16));
    let lens = |on: bool| {
        let fx = Fx::new("lens_distort", json!({ "k1": 0.6, "fit": false, "background": on }));
        fx.input(solid([200; 3])).background("#00ff00").shot()
    };
    let (Some(bent), Some(clamped)) = (lens(true), lens(false)) else { return };
    assert!(green(&bent, 0, 0) && !green(&clamped, 0, 0) && !green(&bent, 32, 16));
    let scroll = |repeat: bool| {
        let fx = Fx::new("scroll", json!({ "speed": [0.25, 0], "repeat_x": repeat }));
        let mut run = fx.input(solid([200; 3])).background("#00ff00").run()?;
        run.runtime.seek(1.0);
        Some(run.shot())
    };
    let (Some(once), Some(again)) = (scroll(false), scroll(true)) else { return };
    assert!(green(&once, 4, 16) && !green(&once, 40, 16) && !green(&again, 4, 16));
    Shot::sheet(&[shown, bent, once]).looks_like("backgrounds");
}

#[test]
fn reflections_turn_their_mirror_line() {
    let mirror = |angle: f32| Fx::new("reflection", json!({ "angle": angle, "amplitude": 0 })).input(ramp()).shot();
    let (Some(flat), Some(turned)) = (mirror(0.0), mirror(30.0)) else { return };
    let Some(plain) = Fx::new("reflection", json!({ "amplitude": 0 })).input(ramp()).shot() else { return };
    assert!(flat.diff(&plain) == 0.0 && turned.diff(&flat) > 1.0, "{}", turned.diff(&flat));
    Shot::sheet(&[flat, turned]).looks_like("reflection_angle");
}

#[test]
fn time_shifts_by_a_phase_picture_and_motions_gain_their_new_shapes() {
    let at = |effect: &str, params: Value, t: f64| {
        let mut run = Fx::new(effect, params).input(squares()).image("phase", phase(), true).run()?;
        run.runtime.seek(t);
        Some(run.shot())
    };
    let cases = [
        ("waves", json!({ "amplitude": 3 }), json!({ "count": 2 }), json!({ "perspective": 1 })),
        ("sway", json!({ "amplitude": 4 }), json!({ "double": true }), json!({ "swing": true, "angle": 10 })),
        ("shake", json!({ "strength": 4 }), json!({ "noise": 1 }), json!({ "friction": 1 })),
        ("flow", json!({ "strength": 6 }), json!({ "noise": true }), json!({ "direction": [0, 1] })),
    ];
    let mut sheet = Vec::new();
    for (effect, base, one, two) in cases {
        let with = |more: &Value| {
            let mut p = base.clone();
            p.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
            p
        };
        let (Some(plain), Some(phased), Some(a), Some(b)) = (
            at(effect, base.clone(), 0.3),
            at(effect, with(&json!({ "phase": "phase" })), 0.3),
            at(effect, with(&one), 0.3),
            at(effect, with(&two), 0.3),
        ) else {
            return;
        };
        let left = (0..W / 2).flat_map(|x| (0..H).map(move |y| (x, y))).all(|(x, y)| phased.at(x, y) == plain.at(x, y));
        assert!(left, "{effect}: the dark half of the phase picture moved");
        differ(&[&plain, &phased, &a, &b], 0.05);
        sheet.extend([plain, phased, a, b]);
    }
    let Some(held) = at("shake", json!({ "strength": 8, "bounds": [0, 0] }), 0.3) else { return };
    assert_eq!(held.diff(&Shot(squares())), 0.0);
    Shot::sheet(&sheet).looks_like("phases");
}

#[test]
fn twirls_stretch_hold_wobble_and_wrap() {
    let twirl = |more: Value| {
        let mut p = json!({ "angle": 120, "radius": 24 });
        p.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
        Fx::new("twirl", p).input(squares()).shot()
    };
    let shots: Vec<Shot> = [json!({}), json!({ "aspect": 3 }), json!({ "inner": 0.6 }), json!({ "noise": 1 })]
        .into_iter()
        .chain([json!({ "repeat": true, "angle": 720, "radius": 80 })])
        .filter_map(twirl)
        .collect();
    let [base, stretched, held, wobbly, wrapped] = &shots[..] else { return };
    differ(&[base, stretched, held, wobbly, wrapped], 0.2);
    Shot::sheet(&shots).looks_like("twirl_shapes");
}
