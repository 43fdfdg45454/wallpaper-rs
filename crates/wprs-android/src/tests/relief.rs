use super::ask;
use crate::{depth, library, store};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::path::Path;

mod checks;

pub(crate) use checks::heif_photos_get_maps;
use checks::masked;

pub(super) fn relief(uid: &str) {
    let deep = |request: Value| serde_json::from_str::<Value>(&depth::request(&request.to_string())).unwrap();
    let model = deep(json!({ "op": "model" }));
    assert_eq!((&model["installed"], &model["allowed"]), (&json!(false), &json!(true)));
    let text = |key: &str| model[key].as_str().unwrap().to_owned();
    assert!(text("size").ends_with(" MB") && text("title").starts_with("Depth Anything V2 Small"));
    assert!(text("hosts").starts_with("github.com") && text("license").starts_with("Apache-2.0"));
    let first = &model["files"][0];
    assert!(first["agent"].as_str().unwrap().starts_with("wallpaper-rs/"));
    assert_eq!(first["bytes"], json!(wprs_depth::MODEL.bytes));
    let part = Path::new(first["part"].as_str().unwrap());
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
    let wrong = deep(json!({ "op": "make", "uid": uid, "device": "abacus" }));
    assert!(wrong["error"].as_str().unwrap().contains("not auto, cpu or cuda"), "{wrong}");
    assert_eq!(deep(json!({ "op": "look", "uid": uid, "colour": true }))["id"], "android.no_depth_map");
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
    assert_eq!(depth::maps::map(&raise).map(|map| map.dimensions()), Some((32, 16)));
    looks(uid, &image, &deep);
    levels(uid, &image, &deep);
    masked(uid, &image);
    assert_eq!(deep(json!({ "op": "unmap", "uid": uid })), json!(1));
    assert_eq!(
        (state(), deep(json!({ "op": "unmap", "uid": uid }))),
        ((json!(true), json!(false)), json!(0))
    );
    assert_eq!(import(32, 16), Value::Null);
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

fn looks(uid: &str, image: &Path, deep: &dyn Fn(Value) -> Value) {
    let shown = |colour: bool| deep(json!({ "op": "look", "uid": uid, "colour": colour }));
    let open = |path: &Value| image::open(path.as_str().unwrap()).unwrap().into_rgb8();
    let grey = shown(false);
    let pixel = open(&grey).get_pixel(0, 0).0;
    let coloured = open(&shown(true));
    assert!(!Path::new(grey.as_str().unwrap()).exists(), "only the shown view is kept");
    assert!(pixel[0] == pixel[1] && pixel[1] == pixel[2], "{pixel:?}");
    assert_ne!(coloured.get_pixel(0, 0).0, pixel, "false colour looks like the grey map");
    assert_eq!(deep(json!({ "op": "item", "uid": uid }))["changed"], json!(false));
    let bytes = fs::read(image).unwrap();
    fs::write(image, [&bytes[..], b"tail"].concat()).unwrap();
    assert_eq!(deep(json!({ "op": "item", "uid": uid }))["changed"], json!(true));
    fs::write(image, bytes).unwrap();
}

fn levels(uid: &str, image: &Path, deep: &dyn Fn(Value) -> Value) {
    let set = json!({ "op": "levels", "uid": uid, "near": 0.8, "far": 0.2, "gamma": 2.0, "smooth": 0, "dilate": 0 });
    assert_eq!(deep(set), Value::Null);
    let item = deep(json!({ "op": "item", "uid": uid }));
    let read = |key: &str| item[key].as_f64().unwrap();
    assert_eq!(
        ["near", "far", "gamma", "smooth", "dilate"].map(read),
        [0.8_f32 as f64, 0.2_f32 as f64, 2.0, 0.0, 0.0]
    );
    let level = depth::maps::map(&depth::raised(image).unwrap()).unwrap().get_pixel(0, 0)[0];
    let expected = ((0.2 + 0.6 * (40_000.0_f32 / 65535.0).powf(2.0)) * 65535.0).round() as u16;
    assert!(level.abs_diff(expected) <= 1, "the near, far and curve shape the map: {level} {expected}");
    let wrong = deep(json!({ "op": "levels", "uid": uid, "gamma": 50.0 }));
    assert!(wrong["error"].is_string(), "{wrong}");
    assert_eq!(
        deep(json!({ "op": "levels", "uid": uid, "near": 1.0, "far": 0.0, "gamma": 1.0 })),
        Value::Null
    );
}
