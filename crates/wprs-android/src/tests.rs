use crate::stage::Stage;
use crate::{choice, depth, library, pack, power, settings, store};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::path::Path;
use wprs_library::Kind;

mod acting;
mod calm;
mod chosen;
mod cued;
mod flight;
mod frames;
mod looks;
mod records;
mod refused;
mod relief;
mod saved;
mod scenes;
mod screens;
mod tilted;
mod words;

pub(super) const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

pub(super) fn ask(request: Value) -> Value {
    serde_json::from_str(&library::request(&request.to_string(), true)).unwrap()
}

pub(super) fn open(name: &str) -> File {
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
        assert_eq!(ask(json!({ "op": "add", "playlist": mix, "items": [item] })), Value::Null);
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
    assert!(choice::commit(true, choice::Target::Both).unwrap());
    assert_eq!(shown(false), bands);
    let listed = ask(json!({ "op": "playlists" }));
    assert_eq!((&listed[0]["count"], &listed[0]["current"]), (&json!(3), &json!(true)));
    assert!(!choice::advance(true, false).unwrap());
    library::with(|lib| {
        let mut row = lib.rotation("home")?.unwrap();
        row.next_at = Some(0);
        Ok(lib.save_rotation(&row)?)
    })
    .unwrap();
    assert!(choice::advance(true, false).unwrap());
    assert_eq!((shown(false), choice::shown(false).map(|s| s.video)), (clip.clone(), Some(true)));
    assert!(choice::colors(false, false).unwrap().is_some_and(|c| c[0] == 0xff00_00ff_u32 as i32));

    let mut info = listed[0]["playlist"].clone();
    info["rules"]["order"] = json!("shuffle");
    assert_eq!(ask(json!({ "op": "save", "playlist": info })), Value::Null);
    assert!(ask(json!({ "op": "save", "playlist": { "uid": mix, "name": "Mix", "query": null, "rules": { "x": 1 } } }))["error"].is_string());
    assert_eq!(ask(json!({ "op": "remove", "uids": [clip] })), Value::Null);
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
    frames::framing(&green, &bands);
    relief::relief(&green);
    records::records();
    scenes::scenes();
    screens::home_and_lock();
    screens::ruled_lists();
    tilted::scenes_follow_the_phone();
    chosen::chosen_files_and_folders();
    refused::packages_the_phone_cannot_play_are_refused();
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

pub(super) fn renderer() -> Option<&'static wprs_render::Renderer> {
    match wprs_render::Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping the scene render: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}
