use super::*;

#[test]
fn particle_systems_name_their_broken_fields() {
    let f = "/particles/fireworks";
    let point = json!({ "at": [0, 0] });
    for (changes, fault) in [
        (
            vec![(format!("{f}/emitters/0/shape/box/at"), json!("p7"))],
            "names the unknown control point \"p7\"".to_string(),
        ),
        (
            vec![(format!("{f}/emitters/1/shape/circle/at"), json!("p9"))],
            "expected p0 to p7, found \"p9\"".into(),
        ),
        (
            vec![(format!("{f}/emitters/0/shape/box/inner"), json!([500, 0]))],
            format!("{f}/emitters/0/shape/box/inner/0 is 500"),
        ),
        (
            vec![(format!("{f}/emitters/0/periodic/on"), json!([0, 1]))],
            format!("{f}/emitters/0/periodic/on/0 is 0"),
        ),
        (
            vec![(format!("{f}/emitters/0/max_per_step"), json!(0))],
            format!("{f}/emitters/0/max_per_step is 0, expected 1..=1000"),
        ),
        (
            vec![(format!("{f}/emitters/1/shape/circle/sign"), json!([2, 0]))],
            format!("{f}/emitters/1/shape/circle/sign/0 is 2"),
        ),
        (
            vec![(format!("{f}/emitters/2/shape/layer/id"), json!("ghost"))],
            "names the unknown layer \"ghost\"".into(),
        ),
        (
            vec![(format!("{f}/emitters/3/shape/around/count"), json!(0))],
            format!("{f}/emitters/3/shape/around/count is 0"),
        ),
        (
            vec![(format!("{f}/init/alpha"), json!([0.5, 2]))],
            format!("{f}/init/alpha/1 is 2, expected 0.5..=1"),
        ),
        (
            vec![(format!("{f}/init/life"), json!([1, 3, 20]))],
            format!("{f}/init/life/2 is 20, expected 0.1..=10"),
        ),
        (
            vec![(format!("{f}/init/hsv"), json!({ "hue": [0, 90] }))],
            format!("{f}/init/palette does not fit"),
        ),
        (
            vec![(format!("{f}/init/palette/colors"), json!([]))],
            format!("{f}/init/palette/colors is 0, expected 1..=10"),
        ),
        (
            vec![(format!("{f}/init/inherit/from"), json!([1, 2]))],
            format!("{f}/init/inherit/from does not fit"),
        ),
        (
            vec![(format!("{f}/init/remap/0/input"), json!("weather"))],
            "names the unknown remap input \"weather\"".into(),
        ),
        (vec![(format!("{f}/init/remap/0/in"), json!([1, 1]))], format!("{f}/init/remap/0/in does not fit")),
        (vec![(format!("{f}/perspective"), Json::Null)], format!("{f}/init/depth does not fit")),
        (
            vec![(format!("{f}/forces/0/window"), json!([0.5, 0.1, 0.8, 1]))],
            format!("{f}/forces/0/window does not fit"),
        ),
        (
            vec![(format!("{f}/forces/2/vortex/outer"), json!(5))],
            format!("{f}/forces/2/outer is 5, expected 10..=100000"),
        ),
        (
            vec![(format!("{f}/forces/4/oscillate/freq"), json!([2, 1]))],
            format!("{f}/forces/4/oscillate/freq/1 is 1"),
        ),
        (vec![(format!("{f}/forces/11/boids/radius"), json!(0))], format!("{f}/forces/11/boids/radius is 0")),
        (vec![(format!("{f}/capacity"), json!(10_000))], format!("{f}/capacity is 10000, expected 1..=8192")),
        (
            vec![(format!("{f}/children/0/system"), json!("fireworks"))],
            format!("{f}/children depends on itself"),
        ),
        (
            vec![(format!("{f}/children/0/system"), json!("ghost"))],
            "names the unknown particle system \"ghost\"".into(),
        ),
        (
            vec![(format!("{f}/children/0/follow_inherit"), json!(true))],
            format!("{f}/children/0/inherit does not fit"),
        ),
        (
            vec![
                ("/particles/smoke/children".into(), json!([{ "system": "spare", "on": "spawn" }])),
                ("/particles/spare".into(), json!({ "capacity": 10, "emitters": [{ "shape": "edge" }] })),
            ],
            format!("{f}/children nests particle systems more than two levels deep"),
        ),
        (vec![(format!("{f}/render/frames"), json!("time"))], format!("{f}/render/cycles does not fit")),
        (vec![(format!("{f}/render/orient"), json!("velocity"))], format!("{f}/render/angle does not fit")),
        (vec![(format!("{f}/render/cutout"), json!([0.8, 0.2, 1]))], format!("{f}/render/cutout/1 is 0.2")),
        (
            vec![(format!("{f}/render/trail/length"), json!(100))],
            format!("{f}/render/trail/length is 100, expected 1..=64"),
        ),
        (
            vec![(format!("{f}/render/stretch"), json!({ "min": 50, "max": 10 }))],
            format!("{f}/render/stretch/max is 10"),
        ),
        (vec![(format!("{f}/render/rope/subdivide"), json!(9))], format!("{f}/render/rope/subdivide is 9")),
        (
            vec![(format!("{f}/collide/0/bounds/box/to"), json!([0, 0]))],
            format!("{f}/collide/0/bounds does not fit"),
        ),
        (
            vec![(format!("{f}/points"), json!(vec![point.clone(); 9]))],
            format!("{f}/points has 9 entries, at most 8"),
        ),
        (vec![(format!("{f}/points/1/at"), json!([0, 0]))], format!("{f}/points/1 needs exactly one source")),
        (vec![(format!("{f}/lock"), json!(["size", "size"]))], format!("{f}/lock does not fit")),
        (
            vec![("/layers/3/instance/count".into(), json!(0.5))],
            "/layers/3/instance/count is locked by its particle system".into(),
        ),
        (
            vec![("/layers/3/instance/brightness".into(), json!(20))],
            "/layers/3/instance/brightness is 20, expected 0..=16".into(),
        ),
        (
            vec![(format!("{f}/emitters"), json!(vec![json!({ "shape": "edge" }); 9]))],
            format!("{f}/emitters has 9 entries, at most 8"),
        ),
    ] {
        let changes: Vec<(&str, Json)> = changes.iter().map(|(p, v)| (p.as_str(), v.clone())).collect();
        expect(&changes, &fault);
    }
}
