use super::*;
use crate::keeping::Keeping;

fn album() -> std::sync::Arc<pack::Pack> {
    album_of().1
}

fn album_of() -> (String, std::sync::Arc<pack::Pack>) {
    let layers = json!([{ "id": "film", "type": "video", "video": "clip" }]);
    let album = json!({ "album": { "type": "folder", "mode": "all", "accept": ["image", "video"] } });
    packed(layers, album)
}

fn packed(layers: Value, properties: Value) -> (String, std::sync::Arc<pack::Pack>) {
    let source = tempfile::tempdir().unwrap();
    let scripts = json!([{ "path": "list.js", "uses": ["folders"] }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [32, 16] }, "layers": layers, "scripts": scripts,
        "assets": { "clip": { "path": "clip.webm" } } });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    fs::write(source.path().join("list.js"), "export function update() {}\n").unwrap();
    fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), source.path().join("clip.webm")).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.album", "version": "1.0.0",
        "title": "Album", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json",
        "content": { "rating": "general" }, "photosensitive": false, "origin": { "kind": "original" },
        "properties": properties });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let wpak = source.path().join("album.wpak");
    wprs_format::build(source.path(), &wpak, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&wpak).unwrap(), "album.wpak").unwrap().0;
    let pack = pack::load(&library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap()).unwrap();
    (uid, pack)
}

pub(super) fn script_folders_list_videos_and_play_the_shown_ones() {
    let pack = album();
    let dir = tempfile::tempdir().unwrap();
    image::RgbaImage::new(2, 2).save(dir.path().join("sea.png")).unwrap();
    fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), dir.path().join("waves.webm")).unwrap();
    let values: wprs_scene::Values = serde_json::from_value(json!({ "album": dir.path() })).unwrap();
    let mut keeping = Keeping::new(&pack, false, true);
    keeping.list((&pack.properties, &pack.scene), &values);
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

pub(super) fn script_folders_are_listed_again_only_when_their_copy_changes() {
    let pack = album();
    let dir = tempfile::tempdir().unwrap();
    image::RgbaImage::new(2, 2).save(dir.path().join("sea.png")).unwrap();
    let values: wprs_scene::Values = serde_json::from_value(json!({ "album": dir.path() })).unwrap();
    let mut keeping = Keeping::new(&pack, false, true);
    keeping.list((&pack.properties, &pack.scene), &values);
    assert!(keeping.text.contains("sea.png"));
    image::RgbaImage::new(2, 2).save(dir.path().join("sand.png")).unwrap();
    keeping.list((&pack.properties, &pack.scene), &values);
    assert!(!keeping.text.contains("sand.png"), "the folder was read again with no change notice");
    crate::chosen::changed();
    keeping.list((&pack.properties, &pack.scene), &values);
    assert!(keeping.text.contains("sand.png"), "a new copy of the folder was not listed");
    let other = tempfile::tempdir().unwrap();
    image::RgbaImage::new(2, 2).save(other.path().join("hill.png")).unwrap();
    let moved: wprs_scene::Values = serde_json::from_value(json!({ "album": other.path() })).unwrap();
    keeping.list((&pack.properties, &pack.scene), &moved);
    assert!(keeping.text.contains("hill.png"), "another chosen folder is listed at once");
}

pub(super) fn folder_videos_get_their_length_and_a_clock_scripts_steer() {
    let (uid, pack) = album_of();
    let dir = tempfile::tempdir().unwrap();
    let film = dir.path().join("waves.webm");
    fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), &film).unwrap();
    let mut stage = crate::stage::Stage::new(uid, pack);
    let key = "wpak-user://album/abc".to_string();
    let shows = wprs_scene::LayerOverride { file: Some(key.clone()), ..Default::default() };
    let pause = wprs_scene::VideoControl { commands: vec![wprs_scene::VideoCommand::Pause] };
    let layers: std::collections::BTreeMap<_, _> = [("film".to_string(), shows)].into();
    let shown = wprs_scene::ScriptOutputs { layers: layers.clone(), ..Default::default() };
    let paused = wprs_scene::ScriptOutputs {
        layers,
        videos: [("film".to_string(), pause)].into(),
        ..Default::default()
    };
    let courses = stage.shown(vec![(key.clone(), film)], vec![shown, paused]);
    let course = courses.iter().find(|c| c.id == "film").expect("the layer runs the folder video");
    assert!(course.duration.is_finite() && course.duration > 0.0 && !course.playing, "{course:?}");
    assert!(stage.videos().iter().any(|(k, _)| *k == key), "the phone opens a player for it");
    let tape = stage.tapes().iter().find(|t| t.asset == key).expect("a clock of its own");
    assert!(tape.shown && !tape.playing && tape.layer.as_deref() == Some("film"), "{tape:?}");
}

pub(super) fn a_video_a_file_property_picks_plays_on_each_own_clock() {
    let layers = json!([{ "id": "a", "type": "video", "video": { "prop": "pick" } },
        { "id": "b", "type": "video", "video": { "prop": "pick" }, "playback": { "own_clock": true } }]);
    let (_, pack) = packed(layers, json!({ "pick": { "type": "file", "accept": ["video"] } }));
    let dir = tempfile::tempdir().unwrap();
    let film = dir.path().join("sea.webm");
    fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), &film).unwrap();
    let values: wprs_scene::Values = serde_json::from_value(json!({ "pick": film })).unwrap();
    let extras = crate::extras::Extras::gather(&pack, (&values, &[]), 0, &Default::default());
    let lane = extras.videos.iter().find(|(k, _)| k == "pick@b").expect("a player for the own clock");
    assert_ne!(lane.1, film, "the own clock gets a player of its own");
    assert_eq!(fs::read(&lane.1).unwrap(), fs::read(&film).unwrap());
}
