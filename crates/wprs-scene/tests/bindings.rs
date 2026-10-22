use serde_json::json;
use wprs_scene::*;

fn sample() -> Scene {
    Scene::from_json(include_str!("sample.json")).unwrap()
}

fn issues(edit: impl FnOnce(&mut Scene)) -> Vec<Issue> {
    let mut scene = sample();
    edit(&mut scene);
    scene.validate()
}

fn range(path: &str, value: f32, min: f32, max: f32) -> Issue {
    Issue::Range { path: path.into(), value, min, max }
}

#[test]
fn property_bindings_resolve_and_apply() {
    let mut scene = sample();
    assert_eq!(scene.get("/layers/3/opacity"), Some(Value::Number(0.3)));
    assert_eq!(scene.get("/layers/4/kind/color"), Some(Value::Color(Color([1.0; 4]))));
    assert_eq!(scene.get("/layers/2/visible"), Some(Value::Bool(true)));
    assert_eq!(scene.get("/layers/5/kind/max_particles"), Some(Value::Number(300.0)));
    assert_eq!(scene.get("/layers/0/effects/2/radius"), Some(Value::Number(0.9)));
    let unbound = ["/layers/9/opacity", "/layers/0/id", "/layers/0/kind/type", "/layers/2/blend", "/size/0"];
    for path in
        unbound.into_iter().chain(["/layers/1/transform/origin", "/layers/0/nope", "layers/0/opacity"])
    {
        assert_eq!(scene.get(path), None, "{path}");
    }
    let red = Color([1.0, 0.0, 0.0, 1.0]);
    if let PropertyKind::Color { value } = &mut scene.properties[0].kind {
        *value = red;
    }
    if let PropertyKind::Bool { value } = &mut scene.properties[2].kind {
        *value = false;
    }
    let applied = scene.with_properties().unwrap();
    assert_eq!(applied.get("/layers/4/kind/color"), Some(Value::Color(red)));
    assert_eq!(applied.get("/layers/3/kind/color"), Some(Value::Color(red)));
    assert!(!applied.layers[2].visible);
    let night = Value::Text("images/city-night.png".into());
    let edited = scene
        .with([("/layers/5/kind/max_particles", Value::Number(120.4)), ("/layers/1/kind/src", night)])
        .unwrap();
    assert!(matches!(&edited.layers[5].kind, LayerKind::Particles(e) if e.max_particles == 120));
    assert!(matches!(&edited.layers[1].kind, LayerKind::Image { src } if src == "images/city-night.png"));
    let wrong = scene.with([("/layers/3/opacity", Value::Bool(true))]);
    assert!(matches!(wrong, Err(SceneError::Binding(path)) if path == "/layers/3/opacity"));
}

#[test]
fn property_rules() {
    let property = |id: &str, kind, targets: &[&str]| -> Property {
        serde_json::from_value(json!({ "id": id, "kind": kind, "targets": targets })).unwrap()
    };
    let slider =
        |value: f32| json!({ "type": "slider", "value": value, "min": 0.0, "max": 1.0, "step": 0.1 });
    let ok = ["/layers/0/opacity", "/layers/5/kind/rate", "/layers/5/kind/max_particles"];
    assert_eq!(issues(|s| s.properties = vec![property("a", slider(0.5), &ok)]), []);
    let off = issues(|s| s.properties = vec![property("a", slider(2.0), &[])]);
    assert_eq!(off, [range("/properties/0/kind/value", 2.0, 0.0, 1.0)]);
    let bad = ["/layers/7/opacity", "/layers/0/visible", "/layers/0/id", "/size/0", "/layers/0/kind/src"];
    let found = issues(|s| s.properties = vec![property("a", slider(0.5), &bad)]);
    let target = |(i, t): (usize, &&str)| (format!("/properties/0/targets/{i}"), t.to_string());
    let binding = |(path, target)| Issue::Binding { path, target };
    assert_eq!(found, bad.iter().enumerate().map(target).map(binding).collect::<Vec<_>>());
    let combo = json!({ "type": "combo", "value": "c.png", "options": ["a.png", "b.png"] });
    let found = issues(|s| s.properties = vec![property("c", combo, &["/layers/0/kind/src"]); 2]);
    let choice = |i: usize| Issue::Choice(format!("/properties/{i}/kind/value"));
    assert_eq!(found, [Issue::DuplicateId("/properties/1/id".into()), choice(0), choice(1)]);
}

#[test]
fn timeline_rules() {
    let (n, black) = (Value::Number, Value::Color(Color::default()));
    let check = |target: &str, keyframes: Vec<(f32, Value)>| {
        let timeline =
            Timeline { target: target.into(), keyframes, easing: Easing::Linear, repeat: Repeat::Loop };
        issues(|s| s.timelines = vec![timeline])
    };
    assert_eq!(check("/layers/0/opacity", vec![(0.0, n(0.0)), (1.0, n(1.0))]), []);
    let unsorted = check("/layers/0/opacity", vec![(1.0, n(0.0)), (1.0, n(1.0))]);
    assert_eq!(unsorted, [Issue::Unsorted("/timelines/0/keyframes".into())]);
    assert_eq!(check("/layers/0/opacity", vec![]), [Issue::Empty("/timelines/0/keyframes".into())]);
    let early = check("/layers/0/opacity", vec![(-1.0, n(0.0))]);
    assert_eq!(early, [range("/timelines/0/keyframes/0/0", -1.0, 0.0, MAX_TIME)]);
    let binding =
        |target: &str| [Issue::Binding { path: "/timelines/0/target".into(), target: target.into() }];
    assert_eq!(check("/layers/0/kind/src", vec![(0.0, n(0.0))]), binding("/layers/0/kind/src"));
    assert_eq!(
        check("/layers/0/opacity", vec![(0.0, n(0.0)), (1.0, black.clone())]),
        binding("/layers/0/opacity")
    );
    let hot = Value::Color(Color([1.0, 0.0, 9.0, 1.0]));
    let found = check("/layers/5/kind/gradient/0", vec![(0.0, black), (1.0, hot)]);
    assert_eq!(found, [range("/timelines/0/keyframes/1/1/2", 9.0, 0.0, 1.0)]);
}
