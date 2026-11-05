use super::*;
use crate::keeping::Keeping;

fn album() -> std::sync::Arc<pack::Pack> {
    let source = tempfile::tempdir().unwrap();
    let scripts = json!([{ "path": "list.js", "uses": ["folders"] }]);
    let layers = json!([{ "id": "film", "type": "video", "video": "clip" }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [32, 16] }, "layers": layers, "scripts": scripts,
        "assets": { "clip": { "path": "clip.webm" } } });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    fs::write(source.path().join("list.js"), "export function update() {}\n").unwrap();
    fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), source.path().join("clip.webm")).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.album", "version": "1.0.0",
        "title": "Album", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json",
        "content": { "rating": "general" }, "photosensitive": false, "origin": { "kind": "original" },
        "properties": { "album": { "type": "folder", "mode": "all", "accept": ["image", "video"] } } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let wpak = source.path().join("album.wpak");
    wprs_format::build(source.path(), &wpak, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&wpak).unwrap(), "album.wpak").unwrap().0;
    pack::load(&library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap()).unwrap()
}

pub(super) fn script_folders_list_videos_and_play_the_shown_ones() {
    let pack = album();
    let dir = tempfile::tempdir().unwrap();
    image::RgbaImage::new(2, 2).save(dir.path().join("sea.png")).unwrap();
    fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), dir.path().join("waves.webm")).unwrap();
    let values: wprs_scene::Values = serde_json::from_value(json!({ "album": dir.path() })).unwrap();
    let mut keeping = Keeping::new(&pack, false, true);
    keeping.list((&pack.properties, &pack.scene), &values, 0.0);
    let listed: Value = serde_json::from_str(&keeping.text).unwrap();
    let rows = listed["album"].as_array().unwrap();
    let kinds: Vec<(&str, &str)> =
        rows.iter().map(|r| (r[2].as_str().unwrap(), r[3].as_str().unwrap())).collect();
    assert!(kinds.contains(&("waves.webm", "video")) && kinds.contains(&("sea.png", "image")), "{kinds:?}");
    let token = rows.iter().find(|r| r[2] == "waves.webm").unwrap()[0].as_str().unwrap();
    let key = format!("wpak-user://album/{token}");
    let mut asked = wprs_scene::ScriptOutputs::default();
    asked
        .layers
        .insert("film".into(), wprs_scene::LayerOverride { file: Some(key.clone()), ..Default::default() });
    assert!(keeping.heard(&mut asked));
    let extras = crate::extras::Extras::gather(&pack, (&values, &keeping.shown), 0, &Default::default());
    assert!(extras.videos.iter().any(|(k, f)| *k == key && f.ends_with("waves.webm")), "{:?}", extras.videos);
}
