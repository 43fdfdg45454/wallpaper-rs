use super::*;
use crate::{reel, variant};

fn built(name: &str, manifest: Value, files: &[(&str, &[u8])]) -> String {
    let source = tempfile::tempdir().unwrap();
    for (file, bytes) in files {
        fs::write(source.path().join(file), bytes).unwrap();
    }
    let mut full = json!({ "schema": wprs_format::SCHEMA, "version": "1.0.0", "title": name, "license": "CC0-1.0",
        "content": { "rating": "general" }, "photosensitive": false, "origin": { "kind": "original" } });
    full.as_object_mut().unwrap().extend(manifest.as_object().unwrap().clone());
    fs::write(source.path().join("manifest.json"), full.to_string()).unwrap();
    let wpak = source.path().join(format!("{name}.wpak"));
    wprs_format::build(source.path(), &wpak, &wprs_format::Limits::default()).unwrap();
    store::store(&mut File::open(&wpak).unwrap(), &format!("{name}.wpak")).unwrap().0
}

pub(super) fn presets_set_the_values_of_their_base() {
    let support = |name| wprs_format::features::support(wprs_format::Platform::Android, name);
    assert_eq!([support("preset"), support("packages")], [wprs_format::features::Support::Runs; 2]);
    let needs = json!([{ "id": "org.example.based", "version": "^1" }]);
    let values = json!({ "base": { "id": "org.example.based", "version": "^1" }, "values": { "speed": 0.5, "volume": 30, "nope": 1 } });
    let preset = built(
        "calm",
        json!({ "id": "org.example.calm", "kind": "preset", "entry": "preset.json",
        "requires": { "packages": needs } }),
        &[("preset.json", values.to_string().as_bytes())],
    );
    let kin = |uid: &str| ask(json!({ "op": "kin", "uid": uid, "languages": [] }));
    let lonely = kin(&preset);
    let wanted = json!({ "id": "org.example.based", "version": "^1", "item": null, "repo": null });
    assert_eq!((&lonely["base"], &lonely["presets"]), (&wanted, &json!([])), "{lonely}");
    let account = |request: Value| -> Value {
        serde_json::from_str(&crate::account::request(&request.to_string())).unwrap()
    };
    let refused = account(json!({ "op": "install_base", "uid": preset }));
    assert_eq!(refused["id"], json!("android.no_release"), "{refused}");
    let error = variant::apply(&preset).unwrap_err();
    assert_eq!(error.reply()["id"], json!("android.preset_needs"), "{error}");
    assert!(
        variant::needed("nowhere", &library::with(|lib| Ok(lib.item(&preset)?.path)).unwrap(), 0).is_empty()
    );
    let clip = fs::read(Path::new(FIXTURES).join("magenta-64x32.webm")).unwrap();
    let base = built(
        "based",
        json!({ "id": "org.example.based", "kind": "video", "entry": "clip.webm",
        "playback": { "speed": 2.0 } }),
        &[("clip.webm", &clip)],
    );
    let path = library::with(|lib| Ok(lib.item(&base)?.path)).unwrap();
    assert_eq!(reel::of(Some(&path)).rate, 2.0);
    assert_eq!(library::with(|lib| variant::preset(lib, &preset)).unwrap(), Some(base.clone()));
    assert_eq!(library::with(|lib| variant::preset(lib, &base)).unwrap(), None);
    assert_eq!(kin(&preset)["base"]["item"], json!({ "uid": base, "title": "based" }));
    let family = kin(&base);
    assert_eq!(
        (&family["base"], &family["presets"]),
        (&Value::Null, &json!([{ "uid": preset, "title": "calm" }]))
    );
    ask(json!({ "op": "pick", "choice": { "item": preset } }));
    assert_eq!(shown(true), base, "a preset previews its base");
    let tried = choice::shown_on(true, false).unwrap();
    assert_eq!(tried.preset.as_deref(), Some(preset.as_str()));
    let previewed = reel::shown(Some(&tried));
    assert_eq!((previewed.rate, previewed.volume), (0.5, 0.3), "the preview shows the values of the preset");
    assert_eq!(reel::of(Some(&path)).rate, 2.0, "previewing a preset changes the settings of its base");
    choice::commit(true, choice::Target::Home).unwrap();
    assert_eq!(shown(false), base, "a preset shows its base with its values");
    let played = reel::of(Some(&path));
    assert_eq!((played.rate, played.volume), (0.5, 0.3));
    assert_eq!(ask(json!({ "op": "remove", "uids": [preset, base] })), Value::Null);
}
