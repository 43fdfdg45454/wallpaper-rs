use super::{FIXTURES, acting, ask, calm, cued, flight, looks, open, renderer, saved};
use crate::stage::Stage;
use crate::{choice, hearing, library, pack, power, store};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::Path;
use wprs_library::Kind;

pub(super) fn scenes() {
    let examples = drawn_examples();
    let added = ask(json!({ "op": "examples", "dir": examples }));
    let uids: Vec<String> = serde_json::from_value(added.clone()).unwrap_or_else(|_| panic!("{added}"));
    assert_eq!(uids.len(), 5);
    let listed = ask(json!({ "op": "items", "query": "kind:scene" }));
    let titles: BTreeSet<_> =
        listed.as_array().unwrap().iter().map(|i| i["title"].as_str().unwrap()).collect();
    assert!(titles.contains("Pulse") && titles.contains("Snowfall"), "{titles:?}");
    super::steady::frames();
    let pulse = listed.as_array().unwrap().iter().find(|i| i["title"] == "Pulse").unwrap();
    let (pulse, path) =
        (pulse["uid"].as_str().unwrap().to_owned(), pulse["path"].as_str().unwrap().to_owned());
    assert!(listed.as_array().unwrap().iter().all(|i| i["thumb"].is_string() && i["width"].is_u64()));
    let refused = ask(json!({ "op": "pick", "choice": { "item": pulse } }));
    assert!(refused["error"].as_str().unwrap().contains("photosensitive"), "{refused}");
    let about = ask(json!({ "op": "item", "uid": pulse, "languages": ["es_ES"] }));
    assert_eq!(
        (&about["package"]["photosensitive"], &about["about"]["title"], &about["title"]),
        (&json!(true), &json!("Pulso"), &json!("Pulso"))
    );
    let spanish = ask(json!({ "op": "items", "query": "kind:scene", "languages": ["es_ES"] }));
    let titles: BTreeSet<_> =
        spanish.as_array().unwrap().iter().map(|i| i["title"].as_str().unwrap()).collect();
    assert!(titles.contains("Pulso") && titles.contains("Colinas al atardecer"), "{titles:?}");
    let lacks = |title: &str| {
        let found = listed.as_array().unwrap().iter().find(|i| i["title"] == title).unwrap();
        ask(json!({ "op": "item", "uid": found["uid"] }))["lacks"].clone()
    };
    assert_eq!(
        [lacks("Orbits"), lacks("Snowfall"), lacks("Pulse")],
        [json!([]), json!(["sound"]), json!([])]
    );
    assert!(
        about["about"]["description"].as_str().unwrap().contains("\n\nDestella con el ritmo y"),
        "{about}"
    );
    assert_eq!(ask(json!({ "op": "confirm", "uid": pulse, "what": "photosensitive" })), Value::Null);
    assert_eq!(ask(json!({ "op": "item", "uid": pulse }))["package"]["confirmed"], json!(["photosensitive"]));
    assert_eq!(ask(json!({ "op": "pick", "choice": { "item": pulse } })), Value::Null);
    ask(json!({ "op": "pick", "choice": null }));
    let dusk = listed.as_array().unwrap().iter().find(|i| i["title"] == "Dusk hills").unwrap();
    let dusk = ask(json!({ "op": "scene", "uid": dusk["uid"], "languages": ["es"] }));
    let labels: Vec<&str> =
        dusk["fields"].as_array().unwrap().iter().filter_map(|f| f["label"].as_str()).collect();
    assert!(labels.contains(&"Momento del día") && !labels.iter().any(|l| l.starts_with('@')), "{labels:?}");
    let form = ask(json!({ "op": "scene", "uid": pulse, "languages": ["es"] }));
    assert_eq!((&form["uses"]["audio"], &form["granted"]["audio"]), (&json!(true), &json!(false)));
    let field = form["fields"].as_array().unwrap()[0].clone();
    let key = field["key"].as_str().unwrap().to_owned();
    assert!(
        ask(json!({ "op": "property", "uid": pulse, "key": "nothing", "value": 1 }))["error"].is_string()
    );
    let scene = pack::load(Path::new(&path)).unwrap();
    let mut stage = Stage::new(pulse.clone(), scene);
    assert_eq!(stage.wants(), [false, false]);
    let charging = crate::power::Power {
        charging: true,
        saver: false,
        low: false,
        thermal: 0,
        headroom: 0.0,
        still: false,
    };
    let r = renderer();
    let draw = |stage: &mut Stage, nanos: i64, cap: u32, force: bool| {
        let r = r.as_ref()?;
        let shot =
            stage.render(r, ((64, 128), nanos, cap), (charging, wprs_scene::Tier::T1), &[], force).unwrap();
        Some((shot.image.map(|i| i.size()), shot.fps, shot.delay))
    };
    if let Some(first) = draw(&mut stage, 0, 60, true) {
        assert!(
            first.0.is_some_and(|(w, h)| w <= 64 && h <= 128) && (first.1, first.2) == (0, 0),
            "{first:?}"
        );
        assert_eq!(draw(&mut stage, 1_000_000, 60, false).map(|shot| shot.0), Some(None));
    }
    assert_eq!(ask(json!({ "op": "grant", "uid": pulse, "input": "audio", "on": true })), Value::Null);
    assert!(ask(json!({ "op": "grant", "uid": pulse, "input": "camera", "on": true }))["error"].is_string());
    assert_eq!(ask(json!({ "op": "scene", "uid": pulse, "languages": [] }))["granted"]["audio"], json!(true));
    stage.refresh();
    assert_eq!(stage.wants(), [true, false]);
    let mut fft = [0i8; hearing::MAX_CAPTURE];
    hearing::hear(&fft, 44_100);
    assert!(!hearing::live());
    if let Some(silent) = draw(&mut stage, 1_000_000, 60, false) {
        assert_eq!(silent.2, 0, "silence lets the scene rest");
        hearing::playback(true);
        assert_eq!(draw(&mut stage, 1_000_000, 60, false).map(|shot| shot.2), Some(2_000));
        hearing::playback(false);
    }
    fft[46] = 100;
    hearing::hear(&fft, 44_100);
    assert!(hearing::live());
    if let Some(heard) = draw(&mut stage, 2_000_000, 60, false) {
        assert!(heard.0.is_some() && heard.1 > 0 && heard.2 > 0, "sound wakes the scene: {heard:?}");
        let still = draw(&mut stage, 3_000_000_000, 0, true).unwrap();
        assert_eq!((still.0.is_some(), still.1), (true, 0));
    }
    if field["kind"] == "slider" || field["kind"] == "bool" {
        let value =
            if field["kind"] == "bool" { json!(field["value"] != json!(true)) } else { field["max"].clone() };
        assert_eq!(ask(json!({ "op": "property", "uid": pulse, "key": key, "value": value })), Value::Null);
        let changed = ask(json!({ "op": "scene", "uid": pulse, "languages": [] }));
        assert_eq!(changed["fields"][0]["value"], value);
        assert_eq!(ask(json!({ "op": "reset", "uid": pulse })), Value::Null);
    }
    let videos = Path::new(FIXTURES).join("magenta-64x32.webm");
    let source = tempfile::tempdir().unwrap();
    fs::copy(&videos, source.path().join("clip.webm")).unwrap();
    let layers = json!([{ "id": "clip", "type": "video", "video": "clip" }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] }, "assets": { "clip": { "path": "clip.webm" } }, "layers": layers });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.clip", "version": "1.0.0", "title": "Clip", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "content": { "rating": "general" }, "photosensitive": false, "origin": { "kind": "original" } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("clip.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let (clip, kind) = store::store(&mut File::open(&built).unwrap(), "clip.wpak").unwrap();
    assert_eq!(kind, Kind::Package);
    let stored = library::with(|lib| Ok(lib.item(&clip)?)).unwrap();
    let loaded = pack::load(&stored.path).unwrap();
    let mut playing = Stage::new(clip.clone(), loaded.clone());
    assert!(playing.plays());
    if let Some(shot) = draw(&mut playing, 0, 60, true) {
        assert!(shot.1 > 0 && shot.2 > 0, "a video layer keeps the scene drawing: {shot:?}");
    }
    let extracted = &loaded.videos[0].1;
    assert_eq!(
        (loaded.videos[0].0.as_str(), fs::read(extracted).unwrap()),
        ("clip", fs::read(&videos).unwrap())
    );
    ask(json!({ "op": "pick", "choice": { "item": clip } }));
    assert!(choice::shown(true).is_some_and(|s| s.scene && !s.video));
    ask(json!({ "op": "pick", "choice": null }));
    assert_eq!(ask(json!({ "op": "remove", "uids": [clip] })), Value::Null);
    assert!(!extracted.exists() && !stored.path.exists());
    let image = store::store(&mut open("green-64x32.png"), "green.png").unwrap().0;
    let bad = library::with(|lib| Ok(lib.item(&image)?.path)).unwrap();
    assert!(pack::load(&bad).is_err());
    saved::saved_settings();
    cued::cued();
    acting::scripted();
    calm::calmed();
    flight::flown();
    looks::drawn();
    power::device(false, 0);
}

fn drawn_examples() -> std::path::PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenes");
    let copy = store::dir().unwrap().join("examples");
    let scenes = fs::read_dir(&source).unwrap().flatten().map(|e| e.path());
    scenes
        .filter(|d| d.join("scene.json").is_file())
        .for_each(|d| tree(&d, &copy.join(d.file_name().unwrap())));
    copy
}

fn tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        match entry.path().is_dir() {
            true => tree(&entry.path(), &target),
            false => drop(fs::copy(entry.path(), target).unwrap()),
        }
    }
}
