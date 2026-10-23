use crate::stage::Stage;
use crate::{account, choice, depth, hearing, library, local, pack, power, settings, store, vault};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::path::Path;
use wprs_library::Kind;
use wprs_sync::{Doc, Hlc, Kept, Replica, Secrets, SyncKey, View};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

fn ask(request: Value) -> Value {
    serde_json::from_str(&library::request(&request.to_string(), true)).unwrap()
}

fn open(name: &str) -> File {
    File::open(Path::new(FIXTURES).join(name)).unwrap()
}

fn shown(preview: bool) -> String {
    choice::shown(preview).map(|shown| shown.uid).unwrap_or_default()
}

#[test]
fn the_library_imports_rotates_and_keeps_settings() {
    let dir = tempfile::tempdir().unwrap().keep();
    fs::copy(Path::new(FIXTURES).join("green-64x32.png"), dir.join("wallpaper")).unwrap();
    fs::write(dir.join("settings.toml"), "video_fit = \"stretch\"\nvideo_sound = \"on\"\n").unwrap();
    store::prepare(dir.to_str().unwrap()).unwrap();
    assert!(!dir.join("wallpaper").exists());
    assert_eq!(
        (settings::chosen("fit"), settings::chosen("playback.muted")),
        ("stretch".into(), "false".into())
    );
    let green = shown(false);
    assert_eq!(choice::shown(false).map(|s| s.video), Some(false));
    assert_eq!(
        store::store(&mut open("green-64x32.png"), "again.png").unwrap(),
        (green.clone(), Kind::Image)
    );
    let (bands, _) = store::store(&mut open("bands-640x180.png"), "Bands.png").unwrap();
    let (clip, kind) = store::store(&mut open("magenta-64x32.webm"), "clip.webm").unwrap();
    assert_eq!(kind, Kind::Video);
    assert!(store::store(&mut "not media".as_bytes(), "notes.txt").is_err());
    assert_eq!(ask(json!({ "op": "thumb", "uid": clip })), Value::Null);
    store::poster(&clip, &[0xff00_00ff_u32 as i32; 8], 4, (64, 32), (1500, false)).unwrap();
    let items = ask(json!({ "op": "items", "query": "" }));
    assert_eq!(items.as_array().map(Vec::len), Some(3));
    let video = &ask(json!({ "op": "items", "query": "kind:video" }))[0];
    assert_eq!(
        (&video["uid"], &video["duration_ms"], video["thumb"].is_string()),
        (&json!(clip), &json!(1500), true)
    );
    let banner = ask(json!({ "op": "item", "uid": bands }));
    assert_eq!(
        (&banner["title"], &banner["width"], &banner["height"]),
        (&json!("Bands"), &json!(640), &json!(180))
    );

    let mix = ask(json!({ "op": "create", "name": "Mix" })).as_str().unwrap().to_owned();
    for item in [&green, &bands, &clip, &green] {
        assert_eq!(ask(json!({ "op": "add", "playlist": mix, "item": item })), Value::Null);
    }
    ask(json!({ "op": "move", "playlist": mix, "from": 0, "to": 2 }));
    let entries = ask(json!({ "op": "entries", "playlist": mix }));
    let order: Vec<_> = entries.as_array().unwrap().iter().map(|i| i["uid"].as_str().unwrap()).collect();
    assert_eq!(order, [&bands, &clip, &green]);
    ask(json!({ "op": "pick", "choice": { "playlist": mix } }));
    assert_eq!((shown(true), shown(false)), (bands.clone(), green.clone()));
    ask(json!({ "op": "unpick", "choice": { "item": bands } }));
    assert_eq!(shown(true), bands);
    ask(json!({ "op": "unpick", "choice": { "playlist": mix } }));
    assert_eq!(shown(true), green);
    ask(json!({ "op": "pick", "choice": { "playlist": mix } }));
    assert!(choice::commit(true).unwrap());
    assert_eq!(shown(false), bands);
    let listed = ask(json!({ "op": "playlists" }));
    assert_eq!((&listed[0]["count"], &listed[0]["current"]), (&json!(3), &json!(true)));
    assert!(!choice::advance(true).unwrap());
    library::with(|lib| {
        let mut row = lib.rotation("home")?.unwrap();
        row.next_at = Some(0);
        Ok(lib.save_rotation(&row)?)
    })
    .unwrap();
    assert!(choice::advance(true).unwrap());
    assert_eq!((shown(false), choice::shown(false).map(|s| s.video)), (clip.clone(), Some(true)));
    assert!(choice::colors(false).unwrap().is_some_and(|c| c[0] == 0xff00_00ff_u32 as i32));

    let mut info = listed[0]["playlist"].clone();
    info["rules"]["order"] = json!("shuffle");
    assert_eq!(ask(json!({ "op": "save", "playlist": info })), Value::Null);
    assert!(ask(json!({ "op": "save", "playlist": { "uid": mix, "name": "Mix", "query": null, "rules": { "x": 1 } } }))["error"].is_string());
    assert_eq!(ask(json!({ "op": "remove", "uid": clip })), Value::Null);
    assert!(!store::thumb_file(&clip, "jpg").unwrap().exists());
    assert_eq!(ask(json!({ "op": "entries", "playlist": mix })).as_array().map(Vec::len), Some(2));
    assert_ne!(shown(false), clip);
    ask(json!({ "op": "delete", "playlist": mix }));
    assert_eq!(ask(json!({ "op": "current" })), json!({ "image": null, "playlist": null }));
    assert!(ask(json!({ "op": "nothing" }))["error"].is_string());

    assert_eq!(settings::setting("fit", Some("fit")).unwrap(), "fit");
    assert_eq!(settings::look().fit, wprs_core::Fit::Fit);
    assert_eq!(settings::setting("system_colors", None).unwrap(), "chosen");
    assert_eq!(settings::setting("system_colors", Some("always")).unwrap(), "always");
    assert!(settings::setting("system_colors", Some("often")).is_err());
    assert!(settings::setting("volume", None).is_err());
    assert_eq!(settings::setting("transition.kind", Some("none")).unwrap(), "none");
    assert!(settings::look().fade.is_zero());
    assert_eq!(settings::setting("playback.max_fps", None).unwrap(), "30");
    relief(&green);
    records();
    scenes();
    unreadable();
}

fn unreadable() {
    let path = store::dir().unwrap().join("config.toml");
    let (saved, broken) = (fs::read(&path).unwrap(), "future_key = 1\n[models]\nallow_download = false\n");
    fs::write(&path, broken).unwrap();
    settings::reload();
    let model: Value = serde_json::from_str(&depth::request(r#"{ "op": "model" }"#)).unwrap();
    assert_eq!(model["allowed"], json!(false));
    assert!(depth::request(r#"{ "op": "install" }"#).contains("turned off"));
    assert!(settings::setting("models.allow_download", None).is_err());
    assert!(settings::setting("transition.kind", Some("fade")).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), broken);
    fs::write(&path, saved).unwrap();
    settings::reload();
    assert!(settings::downloads());
}

fn view(value: Value) -> View {
    serde_json::from_value(value).unwrap()
}

fn records() {
    let ask =
        |request: Value| serde_json::from_str::<Value>(&account::request(&request.to_string())).unwrap();
    assert_eq!(
        (
            ask(json!({ "op": "status" }))["state"].as_str(),
            ask(json!({ "op": "status" }))["active"].as_bool()
        ),
        (Some("none"), Some(false))
    );
    assert!(
        ask(json!({ "op": "pause", "paused": true }))["error"].as_str().unwrap().contains("add a server")
    );
    assert!(
        ask(json!({ "op": "add", "url": "http://example.org" }))["error"].as_str().unwrap().contains("https")
    );
    let mut secrets = Secrets::default();
    secrets.token = "t".into();
    vault::store(&secrets).unwrap();
    assert_eq!(vault::load().unwrap().token, "t");
    vault::forget().unwrap();
    assert!(vault::load().is_err());

    let ids = SyncKey::generate().ids();
    let doc = |value: Value| Doc::stamp(None, Some(&view(value)), |_| false, &Hlc(1, 0, "a".into())).unwrap();
    let section = |name: &str| format!("settings/{}", ids.named("section", name));
    let mut replica = Replica::default();
    replica.bases.insert(
        section("playback"),
        (1, doc(json!({ "playback.max_fps": "display", "playback.volume": 0.2 }))),
    );
    replica.bases.insert("settings/p".into(), (2, doc(json!({ "kind": "profile", "name": "Calm" }))));
    replica.bases.insert("repos/r".into(), (3, doc(json!({ "url": "https://example.org/repo" }))));
    let (mut links, mut kept) = (BTreeMap::new(), Kept::new());
    let (green, _) = store::store(&mut open("green-64x32.png"), "green.png").unwrap();
    let local = local::snapshot(&ids, &replica, &mut links, &BTreeSet::new(), &mut kept).unwrap();
    let echo = |key: &str| replica.bases[key].1.view();
    assert_eq!(
        (local.views.get("settings/p").cloned(), local.views.get("repos/r").cloned()),
        (echo("settings/p"), echo("repos/r"))
    );
    let playback = &local.views[&section("playback")];
    assert_eq!((&playback["playback.max_fps"], playback.get("playback.muted")), (&json!("display"), None));
    let key = local.keys[&green].clone();
    let list = json!({ "name": "Shared", "r:order": "shuffle", "e:a-0": { "i": key, "p": 1.0 }, "e:b-0": { "i": "gone", "p": 2.0 } });
    let applied = vec![
        (
            section("transition"),
            Some(view(json!({ "transition.duration_ms": 900, "playback.max_fps": "display" }))),
        ),
        (format!("library/{key}"), Some(view(json!({ "title": "Desk", "favorite": true, "t:sky": true })))),
        ("playlists/l".into(), Some(view(list))),
        ("settings/p".into(), Some(view(json!({ "kind": "profile", "name": "Calmer" })))),
    ];
    assert_eq!(local::apply(&local, applied, &mut links, &replica, &mut kept), (3, Vec::new()));
    let values =
        ["transition.duration_ms", "playback.max_fps"].map(|key| settings::setting(key, None).unwrap());
    assert_eq!(values, ["900", "30"]);
    let item = library::with(|lib| Ok(lib.item(&green)?)).unwrap();
    assert_eq!((item.title.as_str(), item.favorite, item.tags), ("Desk", true, vec!["sky".to_string()]));
    let shared = links["playlists/l"].clone();
    let entries = library::with(|lib| Ok(lib.playlist_items(&shared, library::now())?)).unwrap();
    assert_eq!(entries.iter().map(|i| i.uid.as_str()).collect::<Vec<_>>(), [green.as_str()]);
    assert_eq!(
        local::apply(&local, vec![("playlists/l".into(), None)], &mut links, &replica, &mut kept).0,
        1
    );
    assert!(library::with(|lib| Ok(lib.playlist(&shared).is_err())).unwrap() && links.is_empty());
}

fn relief(uid: &str) {
    let deep = |request: Value| serde_json::from_str::<Value>(&depth::request(&request.to_string())).unwrap();
    let model = deep(json!({ "op": "model" }));
    assert_eq!(
        (&model["installed"], &model["allowed"], &model["size"]),
        (&json!(false), &json!(true), &json!("99.1 MB"))
    );
    assert_eq!((&model["hosts"], &model["license"]), (&json!("github.com"), &json!("Apache-2.0")));
    assert!(model["agent"].as_str().unwrap().starts_with("wallpaper-rs/"));
    let part = Path::new(model["part"].as_str().unwrap());
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(part, "not a model").unwrap();
    assert!(deep(json!({ "op": "install" }))["error"].as_str().unwrap().contains("SHA-256"));
    assert!(!part.exists());
    assert!(
        deep(json!({ "op": "make", "uid": uid, "library": "/nowhere" }))["error"]
            .as_str()
            .unwrap()
            .contains("not installed")
    );
    assert_eq!(deep(json!({ "op": "item", "uid": uid })), json!({ "parallax": false, "map": false }));
    assert_eq!(deep(json!({ "op": "parallax", "uid": uid, "on": true })), Value::Null);
    let image = library::with(|lib| Ok(lib.item(uid)?.path)).unwrap();
    assert_eq!(depth::raised(&image).map(|raise| raise.map), Some(None));
    let dir = tempfile::tempdir().unwrap();
    let import = |w, h| {
        let map = dir.path().join(format!("{w}x{h}.png"));
        wprs_depth::write(&wprs_depth::Map::from_pixel(w, h, image::Luma([40_000])), &map).unwrap();
        let file = File::open(map).unwrap();
        deep(json!({ "op": "import", "uid": uid, "fd": file.as_raw_fd() }))
    };
    assert!(import(16, 16)["error"].as_str().unwrap().contains("shapes differ"));
    assert_eq!(import(32, 16), Value::Null);
    assert_eq!(deep(json!({ "op": "item", "uid": uid })), json!({ "parallax": true, "map": true }));
    let raise = depth::raised(&image).unwrap();
    assert_eq!(depth::map(&raise).map(|map| map.dimensions()), Some((32, 16)));
    assert_eq!(deep(json!({ "op": "parallax", "uid": uid, "on": false })), Value::Null);
    assert!(depth::raised(&image).is_none());
    if let (Some(model), Ok(runtime)) =
        (std::env::var_os("WPRS_DEPTH_MODEL"), std::env::var("ORT_DYLIB_PATH"))
    {
        fs::create_dir_all(part.parent().unwrap()).unwrap();
        fs::copy(model, part).unwrap();
        assert_eq!(deep(json!({ "op": "install" })), Value::Null);
        for _ in 0..2 {
            assert_eq!(deep(json!({ "op": "make", "uid": uid, "library": runtime })), Value::Null);
        }
        assert!(depth::raised(&image).and_then(|raise| raise.map).is_some());
    }
    assert_eq!(
        [deep(json!({ "op": "forget" })), deep(json!({ "op": "forget" }))],
        [json!(true), json!(false)]
    );
    assert_eq!(ask(json!({ "op": "remove", "uid": uid })), Value::Null);
    assert!(!wprs_depth::beside(&image).exists() && !image.with_added_extension("wprs.json").exists());
    assert!(!fs::read_dir(store::dir().unwrap().join("depth")).is_ok_and(|mut maps| maps.next().is_some()));
}

fn scenes() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenes");
    let added = ask(json!({ "op": "examples", "dir": examples }));
    let uids: Vec<String> = serde_json::from_value(added).unwrap();
    assert_eq!(uids.len(), 5);
    let listed = ask(json!({ "op": "items", "query": "kind:scene" }));
    let titles: BTreeSet<_> =
        listed.as_array().unwrap().iter().map(|i| i["title"].as_str().unwrap()).collect();
    assert!(titles.contains("Pulse") && titles.contains("Snowfall"), "{titles:?}");
    let pulse = listed.as_array().unwrap().iter().find(|i| i["title"] == "Pulse").unwrap();
    let (pulse, path) =
        (pulse["uid"].as_str().unwrap().to_owned(), pulse["path"].as_str().unwrap().to_owned());
    assert!(listed.as_array().unwrap().iter().all(|i| i["thumb"].is_string() && i["width"].is_u64()));
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
    let charging =
        crate::power::Power { charging: true, saver: false, low: false, thermal: 0, headroom: 0.0 };
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
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.clip", "version": "1.0.0", "title": "Clip", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json" });
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
    assert_eq!(ask(json!({ "op": "remove", "uid": clip })), Value::Null);
    assert!(!extracted.exists() && !stored.path.exists());
    let image = store::store(&mut open("green-64x32.png"), "green.png").unwrap().0;
    let bad = library::with(|lib| Ok(lib.item(&image)?.path)).unwrap();
    assert!(pack::load(&bad).is_err());
    power::device(false, 0);
}

fn renderer() -> Option<wprs_render::Renderer> {
    match wprs_render::Renderer::new() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping the scene render: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}
