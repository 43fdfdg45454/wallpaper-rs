use wprs_scene::*;

fn base() -> Scene {
    let mut scene = Scene::new((1920, 1080));
    let image = Layer::new("image", LayerKind::Image { src: "a.png".into() });
    scene.layers = vec![image, Layer::new("dust", LayerKind::Particles(Emitter::default()))];
    scene
}

fn issues(edit: impl FnOnce(&mut Scene)) -> Vec<Issue> {
    let mut scene = base();
    edit(&mut scene);
    scene.validate()
}

fn range(path: &str, value: f32, min: f32, max: f32) -> Issue {
    Issue::Range { path: path.into(), value, min, max }
}

fn emitter(scene: &mut Scene) -> &mut Emitter {
    match &mut scene.layers[1].kind {
        LayerKind::Particles(e) => e,
        _ => unreachable!(),
    }
}

#[test]
fn scene_rules() {
    assert_eq!(base().validate(), []);
    assert_eq!(issues(|s| s.schema = "other/2".into()), [Issue::Schema("other/2".into())]);
    let size = [range("/size/0", 0.0, 1.0, 16384.0), range("/size/1", 20000.0, 1.0, 16384.0)];
    assert_eq!(issues(|s| s.size = (0, 20_000)), size);
    assert_eq!(issues(|s| s.background.0[1] = 2.0), [range("/background/1", 2.0, 0.0, 1.0)]);
    let fill = |i| Layer::new(format!("l{i}"), LayerKind::Color { color: Color::default() });
    let many = issues(|s| s.layers = (0..65).map(fill).collect());
    assert_eq!(many, [Issue::TooMany { path: "/layers".into(), count: 65, max: MAX_LAYERS }]);
    assert_eq!(issues(|s| s.layers[1].id = "image".into()), [Issue::DuplicateId("/layers/1/id".into())]);
    assert_eq!(issues(|s| s.layers[0].id = " ".into()), [Issue::Empty("/layers/0/id".into())]);
}

#[test]
fn layer_rules() {
    let found = issues(|s| {
        s.layers[0].opacity = 1.5;
        s.layers[0].transform.scale = 0.0;
        s.layers[0].mask = Some(String::new());
        s.layers[1].kind =
            LayerKind::Text { text: "hi".into(), font: "Sans".into(), size: -1.0, color: Color([1.0; 4]) };
    });
    let mask = Issue::Empty("/layers/0/mask".into());
    let text = range("/layers/1/kind/size", -1.0, 1.0, 1024.0);
    assert_eq!(
        found,
        [
            range("/layers/0/opacity", 1.5, 0.0, 1.0),
            range("/layers/0/transform/scale", 0.0, 0.001, 1000.0),
            mask,
            text
        ]
    );
    assert!(
        matches!(&issues(|s| s.layers[0].transform.x = f32::NAN)[..], [Issue::Range { path, .. }] if path == "/layers/0/transform/x")
    );
    let found = issues(|s| {
        s.layers[0].kind = LayerKind::Video { src: String::new() };
        s.layers[0].effects = vec![
            Effect::Blur { radius: -1.0 },
            Effect::Tint { color: Color([1.0, 1.0, 3.0, 1.0]), amount: 0.5 },
        ];
    });
    let effects = [
        range("/layers/0/effects/0/radius", -1.0, 0.0, 128.0),
        range("/layers/0/effects/1/color/2", 3.0, 0.0, 1.0),
    ];
    assert_eq!(found, [&[Issue::Empty("/layers/0/kind/src".into())][..], &effects].concat());
    let crowded = issues(|s| s.layers[0].effects = vec![Effect::Pixelate { size: 2.0 }; 17]);
    assert_eq!(crowded, [Issue::TooMany { path: "/layers/0/effects".into(), count: 17, max: MAX_EFFECTS }]);
}

#[test]
fn particle_rules() {
    let found = issues(|s| {
        let e = emitter(s);
        (e.rate, e.lifetime, e.max_particles, e.texture) = (1e5, [3.0, 1.0], 150_000, Some(" ".into()));
        e.gradient.clear();
    });
    assert_eq!(
        found,
        [
            range("/layers/1/kind/rate", 1e5, 0.0, MAX_RATE),
            range("/layers/1/kind/lifetime/1", 1.0, 3.0, 600.0),
            range("/layers/1/kind/max_particles", 150_000.0, 1.0, 100_000.0),
            Issue::Empty("/layers/1/kind/gradient".into()),
            Issue::Empty("/layers/1/kind/texture".into()),
            Issue::ParticleBudget(150_000),
        ]
    );
    let crowd = issues(|s| {
        emitter(s).max_particles = 60_000;
        let twin = Layer { id: "twin".into(), ..s.layers[1].clone() };
        s.layers.push(twin);
    });
    assert_eq!(crowd, [Issue::ParticleBudget(120_000)]);
}
