use wprs_scene::*;

fn sample() -> Scene {
    Scene::from_json(include_str!("sample.json")).unwrap()
}

fn emitter(scene: &Scene) -> &Emitter {
    match &scene.layer("dust").unwrap().kind {
        LayerKind::Particles(e) => e,
        _ => unreachable!(),
    }
}

#[test]
fn effects_declare_cost_and_motion() {
    let waves = |speed| Effect::Waves { amplitude: 2.0, frequency: 1.0, speed, direction: 0.0 };
    let tint = Effect::Tint { color: Color::default(), amount: 0.5 };
    let costs = [Effect::Blur { radius: 4.0 }, Effect::ChromaticAberration { amount: 1.0 }, waves(1.0), tint];
    assert_eq!(costs.map(|e| e.cost()), [Cost::Expensive, Cost::Medium, Cost::Cheap, Cost::Cheap]);
    assert!(Cost::Cheap < Cost::Medium && Cost::Medium < Cost::Expensive);
    let moving = [
        waves(1.0),
        Effect::Scroll { speed_x: 0.0, speed_y: 3.0 },
        Effect::Pulse { amount: 0.2, speed: 1.0 },
    ];
    assert!(moving.iter().all(Effect::animates));
    let still = [
        waves(0.0),
        Effect::Shake { amplitude: 0.0, frequency: 4.0 },
        Effect::Vignette { strength: 1.0, radius: 1.0 },
    ];
    assert!(!still.iter().any(Effect::animates));
}

#[test]
fn static_scenes_need_no_frames() {
    let mut scene = Scene::new((800, 600));
    let glow = Layer::new("glow", LayerKind::Color { color: Color::default() });
    scene.layers = vec![Layer::new("bg", LayerKind::Image { src: "a.png".into() }), glow];
    scene.layers[0].effects.push(Effect::Blur { radius: 3.0 });
    assert!(scene.is_static());
    scene.layers[1].effects.push(Effect::Pulse { amount: 0.2, speed: 1.0 });
    assert_eq!(scene.animated_layers(), [1]);
    scene.layers[1].visible = false;
    assert!(scene.is_static());
    let keyframes = vec![(0.0, Value::Number(1.0)), (1.0, Value::Number(1.0))];
    let target = "/layers/0/opacity".into();
    scene.timelines.push(Timeline { target, keyframes, easing: Easing::Linear, repeat: Repeat::Loop });
    assert!(scene.is_static());
    scene.timelines[0].keyframes[1].1 = Value::Number(0.5);
    assert_eq!(scene.animated_layers(), [0]);
    scene.layers.push(Layer::new("clip", LayerKind::Video { src: "b.webm".into() }));
    assert_eq!(scene.animated_layers(), [0, 2]);
    assert_eq!(sample().animated_layers(), [0, 2, 3, 4, 5]);
}

#[test]
fn particle_defaults_are_conservative() {
    let e = Emitter::default();
    assert!(e.max_particles <= 256 && e.rate <= 16.0);
    assert!(e.rate * e.lifetime[1] <= e.max_particles as f32);
    let mut scene = Scene::new((1280, 720));
    scene.layers.push(Layer::new("p", serde_json::from_str(r#"{ "type": "particles" }"#).unwrap()));
    assert_eq!(scene.layers[0].kind, LayerKind::Particles(Emitter::default()));
    assert_eq!(scene.validate(), []);
}

#[test]
fn quality_lowers_costly_parameters() {
    let scene = sample();
    let lowered = |quality| {
        let mut s = scene.clone();
        s.apply_quality(quality);
        s
    };
    assert_eq!(lowered(Quality::High), scene);
    assert_eq!(Quality::default(), Quality::High);
    for (quality, rate, max, radius) in [(Quality::Medium, 12.0, 150, 1.0), (Quality::Low, 6.0, 75, 0.5)] {
        let s = lowered(quality);
        assert_eq!((emitter(&s).rate, emitter(&s).max_particles), (rate, max), "{quality:?}");
        assert_eq!(s.layer("skyline").unwrap().effects[0], Effect::Blur { radius }, "{quality:?}");
        assert_eq!(s.validate(), [], "{quality:?}");
    }
    assert!(Quality::Low < Quality::Medium && Quality::Medium < Quality::High);
}
