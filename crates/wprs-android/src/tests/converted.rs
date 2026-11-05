use super::{FIXTURES, ask, open};
use crate::{library, settings, store};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

fn path(uid: &str) -> PathBuf {
    library::with(|lib| Ok(lib.item(uid)?.path)).unwrap()
}

pub(super) fn converted_videos_take_the_place_of_their_original() {
    assert_eq!(settings::setting("video_convert", None).unwrap(), "auto");
    assert!(settings::setting("video_convert", Some("always")).is_err());
    assert_eq!(settings::setting("video_convert", Some("keep")).unwrap(), "keep");
    assert_eq!(settings::setting("video_convert", Some("auto")).unwrap(), "auto");
    let (clip, _) = store::store(&mut open("magenta-64x32.webm"), "convert.webm").unwrap();
    let (still, _) = store::store(&mut open("green-64x32.png"), "convert.png").unwrap();
    for wrong in [still.as_str(), "../media/x", "", "gone"] {
        let refused = ask(json!({ "op": "convert_to", "uid": wrong }));
        assert_eq!(refused["id"], json!("android.convert_item"), "{wrong}: {refused}");
    }
    let stale = store::dir().unwrap().join("converting/stale.mp4");
    fs::create_dir_all(stale.parent().unwrap()).unwrap();
    fs::write(&stale, b"cut short").unwrap();
    let target = ask(json!({ "op": "convert_to", "uid": clip }));
    let target = PathBuf::from(target.as_str().unwrap_or_else(|| panic!("{target}")));
    assert!(!stale.exists(), "a conversion cut short in an earlier run was left behind");
    assert!(target.ends_with(format!("converting/{clip}.mp4")) && target.parent().unwrap().is_dir());
    assert_eq!(ask(json!({ "op": "converted", "uid": clip }))["id"], json!("android.convert_failed"));
    fs::write(&target, b"not a video").unwrap();
    assert_eq!(ask(json!({ "op": "converted", "uid": clip }))["id"], json!("android.convert_failed"));
    assert!(!target.exists(), "an unreadable conversion is thrown away");
    let before = path(&clip);
    fs::copy(Path::new(FIXTURES).join("quadrants-64x32.webm"), &target).unwrap();
    assert_eq!(ask(json!({ "op": "unconverted", "uid": clip })), Value::Null);
    assert!(!target.exists());
    fs::copy(Path::new(FIXTURES).join("quadrants-64x32.webm"), &target).unwrap();
    let placed = ask(json!({ "op": "converted", "uid": clip }));
    let after = path(&clip);
    assert_eq!(placed, json!(after));
    assert!(after != before && after.extension().unwrap() == "mp4" && after.is_file() && !before.exists());
    assert!(!target.exists() && !store::thumb_file(&clip, "jpg").unwrap().exists());
    assert_eq!(ask(json!({ "op": "convert_to", "uid": clip })), json!(target));
    fs::write(&target, b"half").unwrap();
    assert_eq!(ask(json!({ "op": "remove", "uids": [clip, still] })), Value::Null);
    assert!(!target.exists(), "removing a video leaves its conversion behind");
}
