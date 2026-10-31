use super::*;

pub(super) fn drawn() {
    let source = tempfile::tempdir().unwrap();
    let floor =
        image::RgbaImage::from_fn(16, 8, |_, y| image::Rgba([255, 255, 255, if y >= 5 { 255 } else { 0 }]));
    floor.save(source.path().join("floor.png")).unwrap();
    let rain = json!({ "capacity": 60, "seed": 3,
        "emitters": [{ "shape": { "box": { "from": [4, 0], "to": [60, 4] } }, "rate": 60 }],
        "init": { "life": [2, 2], "speed": [0, 0], "size": [3, 3], "gradient": [[0, "#ff0000"], [1, "#0000ff"]] },
        "forces": [{ "gravity": [0, 200] }],
        "collide": [{ "bounds": { "mask": { "texture": "floor" } }, "bounce": 0.2 }],
        "render": { "order": "newest", "trail": { "length": 6, "width": 0.5 } } });
    let glow = json!({ "audio": [0, 3], "of": 16, "reduce": "max", "bounds": [0.1, 0.9], "exponent": 2,
        "smooth": { "up": 0.05, "down": 0.5 }, "offset": 0.5 });
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32], "background": "#000000" },
        "assets": { "floor": { "path": "floor.png" } }, "audio": { "attack": 0.05, "release": 0.3, "gain": "fixed" },
        "layers": [{ "id": "rain", "type": "particles", "system": "rain", "blend": "normal", "opacity": glow }],
        "particles": { "rain": rain } });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.rain", "version": "1.0.0", "title": "Rain",
        "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" }, "inputs": ["audio"] });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("rain.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let package = wprs_format::open(&built, &wprs_format::Limits::default()).unwrap();
    let used: Vec<&str> = package.facts.features.iter().map(String::as_str).collect();
    assert_eq!(used, ["audio_ranges", "audio_response", "inputs.audio", "particles2", "shaping"]);
    assert!(wprs_format::features::refused("android", &package.facts.features).is_empty());
    let uid = store::store(&mut File::open(&built).unwrap(), "rain.wpak").unwrap().0;
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    let power =
        power::Power { charging: true, saver: false, low: false, thermal: 0, headroom: 0.0, still: false };
    if let Some(r) = renderer() {
        let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
        for n in 0..20 {
            let at = ((64, 32), n * 1_000_000_000 / 30, 60);
            assert!(stage.render(r, at, (power, wprs_scene::Tier::T3), &[], true).unwrap().fps > 0);
        }
        let path = if r.compute() && !r.software() { "gpu" } else { "cpu" };
        assert_eq!(crate::stage::report::particles(), path);
    }
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
