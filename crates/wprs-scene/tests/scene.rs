use wprs_scene::{Color, EFFECTS_WGSL, Effect, LayerKind, Scene};

const SAMPLE: &str = include_str!("sample.json");

fn sample() -> Scene {
    Scene::from_json(SAMPLE).unwrap()
}

#[test]
fn sample_roundtrips_and_is_valid() {
    let scene = sample();
    assert_eq!(scene.validate(), []);
    assert_eq!(Scene::from_json(&scene.to_json().unwrap()).unwrap(), scene);
    let title = scene.layer("title").unwrap();
    assert_eq!(
        (title.transform.scale, title.transform.origin, title.opacity, title.visible),
        (1.0, [0.5; 2], 1.0, true)
    );
    assert!(matches!(&scene.layer("dust").unwrap().kind, LayerKind::Particles(e) if e.max_particles == 300));
}

#[test]
fn unknown_fields_and_kinds_are_rejected() {
    let edits = [
        ("\"opacity\": 0.6", "\"opacitty\": 0.6"),
        ("\"radius\": 2.0", "\"radius\": 2.0, \"sigma\": 1.0"),
        ("\"type\": \"blur\"", "\"type\": \"bloom\""),
        ("\"max_particles\": 300", "\"max_particles\": 300, \"wind\": 1"),
        ("\"step\": 1.0", "\"step\": 1.0, \"unit\": \"%\""),
        ("\"repeat\": \"loop\"", "\"repeat\": \"forever\""),
    ];
    for (from, to) in edits {
        assert!(SAMPLE.contains(from), "{from}");
        assert!(Scene::from_json(&SAMPLE.replacen(from, to, 1)).is_err(), "{to}");
    }
}

#[test]
fn assets_include_bound_alternatives() {
    let scene = sample();
    let assets: Vec<&str> = scene.assets().into_iter().collect();
    let expected = ["images/city-night.png", "images/city.png", "images/sky.jpg", "masks/city.png"];
    assert_eq!(assets, [&expected[..], &["textures/dot.png", "videos/rain.webm"]].concat());
}

fn compile(source: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let flags = naga::valid::ValidationFlags::all();
    let mut validator = naga::valid::Validator::new(flags, naga::valid::Capabilities::default());
    validator.validate(&module).map(drop).map_err(|e| e.emit_to_string(source))
}

#[test]
fn every_effect_shader_compiles() {
    let effects: Vec<Effect> = sample().layers.into_iter().flat_map(|l| l.effects).collect();
    let mut names: Vec<&str> = effects.iter().map(Effect::name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 10);
    compile(EFFECTS_WGSL).unwrap();
    assert!(compile("fn broken(").is_err());
    for effect in &effects {
        compile(&effect.shader()).unwrap_or_else(|e| panic!("{}: {e}", effect.name()));
        assert_eq!(serde_json::to_value(effect).unwrap()["type"], effect.name());
    }
    let tint = Effect::Tint { color: Color([0.1, 0.2, 0.3, 0.4]), amount: 0.5 };
    assert_eq!(tint.uniforms(), [0.1, 0.2, 0.3, 0.4, 0.5, 0.0, 0.0, 0.0]);
}
