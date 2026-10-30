use super::*;

pub(super) fn flown() {
    let source = tempfile::tempdir().unwrap();
    let systems = json!({
        "fuse": { "capacity": 20, "space": "world", "emitters": [{ "shape": { "point": [32, 16] }, "burst": { "count": 4 } }],
            "init": { "life": [0.1, 0.1], "speed": [0, 0], "size": [3, 3] }, "children": [{ "system": "spark", "on": "death" }] },
        "spark": { "capacity": 200, "emitters": [{ "shape": { "circle": { "center": [0, 0], "radius": 6 } }, "burst": { "count": 10 } }],
            "init": { "life": [0.3, 0.3], "speed": [0, 0], "size": [4, 4] } }
    });
    let layers = json!([{ "id": "p", "type": "particles", "system": "fuse", "blend": "add", "instance": { "color": "#ff0000" } }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32], "background": "#000000" },
        "layers": layers, "particles": systems });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.fuse", "version": "1.0.0", "title": "Fuse",
        "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("fuse.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&built).unwrap(), "fuse.wpak").unwrap().0;
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    let power =
        power::Power { charging: true, saver: false, low: false, thermal: 0, headroom: 0.0, still: false };
    if let Some(r) = renderer() {
        let mut stage = Stage::new(uid.clone(), pack::load(&path).unwrap());
        let fps: Vec<u32> = (0..30)
            .map(|n| {
                let at = ((64, 32), n * 1_000_000_000 / 30, 60);
                stage.render(r, at, (power, wprs_scene::Tier::T3), &[], true).unwrap().fps
            })
            .collect();
        assert!(fps[0] > 0 && fps[5] > 0, "the fuse and its sparks move: {fps:?}");
        assert_eq!(fps[29], 0, "the scene sleeps once the sparks are gone: {fps:?}");
    }
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
