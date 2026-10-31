use super::ask;
use crate::{depth, framing, library, store};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::path::Path;

pub(super) fn relief(uid: &str) {
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
    let state = || {
        let item = deep(json!({ "op": "item", "uid": uid }));
        (item["parallax"].clone(), item["map"].clone())
    };
    assert_eq!(state(), (json!(false), json!(false)));
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
    assert_eq!(state(), (json!(true), json!(true)));
    let (reader, mut writer) = std::io::pipe().unwrap();
    let request = json!({ "op": "import", "uid": uid, "fd": reader.as_raw_fd() });
    let importing = std::thread::spawn(move || {
        let answer = deep(request);
        drop(reader);
        answer
    });
    assert_eq!(deep(json!({ "op": "fit", "uids": [uid], "fit": "stretch" })), json!(1));
    std::io::Write::write_all(&mut writer, &fs::read(dir.path().join("32x16.png")).unwrap()).unwrap();
    drop(writer);
    assert_eq!(importing.join().unwrap(), Value::Null);
    let after = depth::loaded(&image).unwrap();
    assert_eq!(
        (after.fit, after.moving().is_some()),
        (Some(wprs_core::Fit::Stretch), true),
        "a map keeps what changed while it was read"
    );
    let raise = depth::raised(&image).unwrap();
    assert_eq!(depth::map(&raise).map(|map| map.dimensions()), Some((32, 16)));
    masked(uid, &image);
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
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
    assert!(!wprs_depth::beside(&image).exists() && !image.with_added_extension("wprs.json").exists());
    assert!(!fs::read_dir(store::dir().unwrap().join("depth")).is_ok_and(|mut maps| maps.next().is_some()));
}

fn masked(uid: &str, image: &Path) {
    let edit = |value: Value| wprs_core::Edit::from_value(value).unwrap();
    let tinted = edit(json!({"version": 1, "geometry": {"rotate": 90},
        "masks": {"near": {"type": "depth", "range": [0.5, 1.0], "soft": 0.1}},
        "ops": [{"id": "ev", "type": "exposure", "ev": 1, "mask": "near"}]}));
    let before = depth::loaded(image).unwrap();
    depth::change(image, |e| {
        (e.geometry, e.masks, e.ops) = (tinted.geometry.clone(), tinted.masks.clone(), tinted.ops.clone());
        Ok(())
    })
    .unwrap();
    let framed = framing::framed(uid, image, wprs_core::Fit::Fill, false);
    let develop = framed.develop.as_ref().unwrap();
    let map = depth::masking(framed.map.as_ref().map(|(path, _)| path.as_path()), develop);
    assert_eq!(
        (develop.needs_depth(), develop.geometry.rotate, map.map(|m| m.dimensions())),
        (true, 90, Some((32, 16))),
        "a turned photo with a depth mask is developed with its map"
    );
    depth::change(image, |e| {
        (e.geometry, e.masks, e.ops) = (before.geometry, before.masks, before.ops);
        Ok(())
    })
    .unwrap();
    assert!(framing::framed(uid, image, wprs_core::Fit::Fill, false).map.is_none());
}
