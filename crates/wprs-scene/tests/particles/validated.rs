use super::*;

#[test]
fn systems_are_validated_against_their_limits() {
    let range =
        |path: &str, value: f32, min: f32, max: f32| Issue::Range { path: path.into(), value, min, max };
    let p = "/particles/dots";
    assert_eq!(issues(sparks()), []);
    for bad in [
        json!({ "forces": [{ "attractor": { "target": "mouse", "strength": 1 } }] }),
        json!({ "emitters": [{ "shape": "edge", "burst": { "count": 1, "on": "clap" } }] }),
        json!({ "render": { "frames": "sometimes" } }),
        json!({ "collide": [{ "bounds": "box" }] }),
        json!({ "init": { "glow": [1, 2] } }),
    ] {
        assert!(Scene::from_json(&scene(with(sparks(), bad.clone()), true)).is_err(), "{bad}");
    }
    assert_eq!(
        issues(with(sparks(), json!({ "capacity": 0 }))),
        [range(&format!("{p}/capacity"), 0.0, 1.0, 200_000.0)]
    );
    assert_eq!(
        issues(with(sparks(), json!({ "prewarm": 40 }))),
        [range(&format!("{p}/prewarm"), 40.0, 0.0, 30.0)]
    );
    let life = with(sparks(), json!({ "init": { "life": [3, 1] } }));
    assert_eq!(issues(life), [range(&format!("{p}/init/life/1"), 1.0, 3.0, 600.0)]);
    let five = Json::Array(vec![json!({ "shape": "edge", "rate": 1 }); 5]);
    let wide =
        Scene::from_json(&scene(with(sparks(), json!({ "emitters": five, "prewarm": 20 })), true)).unwrap();
    assert!(wide.validate(&props()).is_empty() && wide.features().contains("particles2"));
    let nine = Json::Array(vec![json!({ "shape": "edge", "rate": 1 }); 9]);
    let too_many = Issue::TooMany { path: format!("{p}/emitters"), count: 9, max: 8 };
    assert_eq!(issues(with(sparks(), json!({ "emitters": nine }))), [too_many]);
    let bouncy = with(sparks(), json!({ "collide": [{ "bounds": "canvas", "bounce": 2 }] }));
    assert_eq!(issues(bouncy), [range(&format!("{p}/collide/0/bounce"), 2.0, 0.0, 1.0)]);
    let gone = Issue::Unknown { path: format!("{p}/render/texture"), what: "asset", id: "gone".into() };
    assert_eq!(issues(with(sparks(), json!({ "render": { "texture": "gone" } }))), [gone]);
    let rate = json!([{ "shape": "edge", "rate": { "prop": "nothing" } }]);
    let unknown =
        Issue::Unknown { path: format!("{p}/emitters/0/rate"), what: "property", id: "nothing".into() };
    assert_eq!(issues(with(sparks(), json!({ "emitters": rate }))), [unknown]);
    let curve = with(sparks(), json!({ "render": { "size_over_life": [[0.5, 1], [0.1, 0]] } }));
    assert_eq!(issues(curve), [Issue::Unsorted(format!("{p}/render/size_over_life"))]);
    let mut big = Scene::from_json(&scene(sparks(), true)).unwrap();
    let huge: System = serde_json::from_value(with(sparks(), json!({ "capacity": 150_000 }))).unwrap();
    big.particles.extend((0..32).map(|i| (format!("s{i}"), huge.clone())));
    let found = big.validate(&props());
    assert_eq!(found[0], Issue::TooMany { path: "/particles".into(), count: 33, max: 32 });
    assert!(matches!(&found[1], Issue::Range { path, value, .. } if path == "/particles" && *value > 1e6));
    let textured = with(
        sparks(),
        json!({ "render": { "texture": "dot", "frames": "life", "color_over_life": [[0, "#ffffff"], [1, "#ff000000"]], "trail": { "length": 8, "width": 0.5 } } }),
    );
    assert_eq!(issues(textured.clone()), []);
    let system: System = serde_json::from_value(textured).unwrap();
    let (sizes, colors) = system.render.luts();
    assert_eq!((sizes[0], sizes[63], colors[0], colors[63]), (1.0, 1.0, [1.0; 4], [1.0, 0.0, 0.0, 0.0]));
    assert!((colors[32][1] - (1.0 - 32.0 / 63.0)).abs() < 1e-5);
}

#[test]
fn the_cpu_simulation_is_deterministic_and_follows_quality() {
    let text = scene(sparks(), true);
    let a = run(&text, 120, Quality::Q3);
    assert_eq!(a.particles, run(&text, 120, Quality::Q3).particles);
    let census = Census::of(&a.particles);
    assert!(census.alive > 100 && census.alive == a.alive(), "{census:?}");
    assert!(census.min[0] >= 0.0 && census.max[0] <= 200.0 && census.min[1] >= 0.0 && census.max[1] <= 100.0);
    assert!(census.mean[1] < 90.0, "particles rise: {census:?}");
    assert_ne!(a.particles, run(&text.replace("\"seed\":3", "\"seed\":4"), 120, Quality::Q3).particles);
    let low = Census::of(&run(&text, 120, Quality::Q0).particles);
    assert!(low.alive <= 125 && low.alive * 3 < census.alive, "{low:?}");
}
