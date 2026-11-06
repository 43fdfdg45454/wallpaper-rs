use crate::stall::request;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use wprs_format::{
    Kind, Limits, Manifest, SCHEMA_V1, Settings, Signer, Text, build, init, keygen, sign_root,
};

fn package(dir: &Path, id: &str) -> PathBuf {
    let src = dir.join(format!("src-{id}"));
    fs::create_dir_all(&src).unwrap();
    let manifest = Manifest {
        schema: SCHEMA_V1.into(),
        id: id.into(),
        version: "1.0.0".into(),
        title: Text::Plain("Sky".into()),
        description: "A calm gradient".into(),
        license: "CC0-1.0".into(),
        kind: Kind::Image,
        entry: "image.png".into(),
        ..Default::default()
    };
    fs::write(src.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    fs::write(src.join("image.png"), id).unwrap();
    let out = dir.join(format!("{id}.wpak"));
    build(&src, &out, &Limits::default()).unwrap();
    out
}

fn ask(dir: &Path, op: Value) -> Value {
    let mut op = op;
    op["dir"] = json!(dir);
    serde_json::from_str(&request(&op.to_string())).unwrap()
}

#[test]
fn a_static_repository_folder_is_operated_from_the_phone_and_names_each_file_it_changed() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("repo");
    let (root, online) = (keygen(None).unwrap(), keygen(Some("example".into())).unwrap());
    let settings = Settings {
        name: "Example wallpapers".into(),
        root_keys: vec![root.public.clone()],
        online_keys: vec![online.public],
        ..Settings::default()
    };
    init(&dir, &settings, 1_800_000_000).unwrap();
    let opened = ask(&dir, json!({"op": "open"}));
    assert_eq!(
        (&opened["name"], &opened["packages"], &opened["changed"]),
        (&json!("Example wallpapers"), &json!([]), &json!([]))
    );
    let file = package(tmp.path(), "org.example.dawn");
    let added = ask(&dir, json!({"op": "add", "files": [file], "unlisted": true}));
    assert_eq!(added["added"], json!(["org.example.dawn 1.0.0"]));
    assert_eq!(added["packages"][0]["versions"][0]["listed"], false);
    let changed = added["changed"].as_array().unwrap();
    assert!(changed.contains(&json!("catalog.json")), "{changed:?}");
    assert!(changed.contains(&json!("packages/org.example.dawn/1.0.0.wpak")), "{changed:?}");
    let listed = ask(&dir, json!({"op": "list", "id": "org.example.dawn", "listed": true}));
    assert_eq!(
        (&listed["packages"][0]["versions"][0]["listed"], &listed["changed"]),
        (&json!(true), &json!(["catalog.json"]))
    );
    let picked =
        ask(&dir, json!({"op": "feature", "id": "org.example.dawn", "note": "Calm morning", "on": true}));
    assert_eq!(picked["packages"][0]["note"], "Calm morning");
    let missing = ask(&dir, json!({"op": "feature", "id": "org.example.none", "on": true}));
    assert!(missing["error"].is_string(), "{missing}");
    let unsigned =
        ask(&dir, json!({"op": "publish", "key": online.secret, "password": "example", "days": 7}));
    assert!(unsigned["error"].is_string(), "{unsigned}");
    sign_root(&dir, &Signer::new(&root.secret, None).unwrap()).unwrap();
    let wrong = ask(&dir, json!({"op": "publish", "key": online.secret, "password": "wrong", "days": 7}));
    assert!(wrong["error"].is_string(), "{wrong}");
    let published =
        ask(&dir, json!({"op": "publish", "key": online.secret, "password": "example", "days": 7}));
    assert_eq!(published["published"]["packages"], 1);
    let changed = published["changed"].as_array().unwrap();
    assert!(
        changed.contains(&json!("entry/latest")) && changed.contains(&json!("entry/1.json.minisig")),
        "{changed:?}"
    );
    assert!(!changed.iter().any(|c| c.as_str().unwrap().starts_with("packages/")), "{changed:?}");
}
