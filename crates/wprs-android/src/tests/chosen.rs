use super::{FIXTURES, ask, open};
use crate::extras::Extras;
use crate::{library, pack, props, store};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::path::Path;

pub(super) fn chosen_files_and_folders() {
    let source = tempfile::tempdir().unwrap();
    fs::copy(Path::new(FIXTURES).join("green-64x32.png"), source.path().join("sky.png")).unwrap();
    let layers = json!([{ "id": "sky", "type": "image", "image": "sky" }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] },
        "assets": { "sky": { "path": "sky.png" } }, "layers": layers });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let properties = json!({ "photo": { "type": "file", "default_asset": "sky" },
        "album": { "type": "folder", "every": 60, "pick": "sequence", "accept": ["image"] } });
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.album", "version": "1.0.0",
        "title": "Album", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json",
        "content": { "rating": "general" }, "photosensitive": false, "origin": { "kind": "original" },
        "properties": properties, "requires": { "features": ["folder", "properties2"] } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("album.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&built).unwrap(), "album.wpak").unwrap().0;
    let path = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    let pack = pack::load(&path).unwrap();
    let bands = open("bands-640x180.png");
    let chosen = ask(
        json!({ "op": "choose", "uid": uid, "key": "photo", "fd": bands.as_raw_fd(), "name": "bands.png" }),
    );
    assert!(Path::new(chosen.as_str().unwrap()).is_file(), "{chosen}");
    let clip = open("magenta-64x32.webm");
    let refused = ask(
        json!({ "op": "choose", "uid": uid, "key": "photo", "fd": clip.as_raw_fd(), "name": "clip.webm" }),
    );
    assert_eq!(refused["id"], json!("android.not_accepted"), "an image property takes no video");
    let gathered = |at| Extras::gather(&pack, (&props::values(&pack), &[]), at, &Extras::default());
    let sky = gathered(0).assets(&pack).images["sky"].size();
    assert_eq!(sky, (640, 180), "the chosen picture replaces the default one");
    let plan = ask(json!({ "op": "folder", "uid": uid, "key": "album", "tree": "content://tree/photos" }));
    let dir = Path::new(plan["dir"].as_str().unwrap()).to_owned();
    assert_eq!((&plan["accept"], &plan["max"]), (&json!(["image"]), &json!(1000)));
    assert_eq!(
        ask(json!({ "op": "folded", "uid": uid, "key": "album" }))["id"],
        json!("android.folder_nothing")
    );
    for name in ["a.png", "b.png"] {
        fs::copy(Path::new(FIXTURES).join("green-64x32.png"), dir.join(name)).unwrap();
    }
    fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), dir.join("c.webm")).unwrap();
    assert_eq!(ask(json!({ "op": "folded", "uid": uid, "key": "album" })), json!(dir));
    let mirrors = ask(json!({ "op": "mirrors" }));
    assert_eq!(mirrors[0]["plan"]["tree"], json!("content://tree/photos"));
    let (first, second) = (gathered(0), gathered(60));
    assert_eq!((first.next, second.next), (Some(60), Some(120)), "a folder turns at its own pace");
    assert!(!first.same(&second) && first.same(&gathered(59)));
    assert_eq!(first.assets(&pack).images["album"].size(), (64, 32));
    let released = ["photo", "album"].map(|key| ask(json!({ "op": "unchoose", "uid": uid, "key": key })));
    assert_eq!(
        released,
        [Value::Null, json!("content://tree/photos")],
        "the folder hands back its permission"
    );
    assert!(!dir.exists() && ask(json!({ "op": "mirrors" })) == json!([]));
    assert_eq!(gathered(0).assets(&pack).images["sky"].size(), (64, 32));
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}

#[test]
fn mature_packages_in_repositories_are_labelled_and_hidden_when_asked() {
    let rated = |id: &str, rating: Option<&str>| wprs_format::Item {
        repo_id: "r".into(),
        repo_name: "Studio".into(),
        id: id.into(),
        author: None,
        release: wprs_format::Release {
            content: rating.map(|r| wprs_format::Rated { rating: r.into(), descriptors: Vec::new() }),
            ..Default::default()
        },
        versions: Vec::new(),
        installed: Vec::new(),
    };
    let found = || vec![rated("calm", Some("general")), rated("storm", Some("mature")), rated("old", None)];
    let shown = |hide| {
        let listed = crate::repos::listed(found(), hide, &[]);
        let row = |i: &Value| (i["id"].as_str().unwrap().to_owned(), i["mature"].as_bool().unwrap());
        listed.as_array().unwrap().iter().map(row).collect::<Vec<_>>()
    };
    let all = [("calm".to_owned(), false), ("storm".into(), true), ("old".into(), false)];
    assert_eq!(shown(false), all);
    assert_eq!(shown(true), [all[0].clone(), all[2].clone()]);
}
