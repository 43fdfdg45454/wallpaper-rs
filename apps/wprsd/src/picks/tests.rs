use super::*;
use serde_json::json;
use std::fs;

fn folder(extra: serde_json::Value) -> Property {
    let mut prop = json!({ "type": "folder", "accept": ["image"] });
    prop.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(prop).unwrap()
}

#[test]
fn turns_of_hidden_wallpapers_wait_until_they_show() {
    let now = Instant::now();
    let later = now + Duration::from_secs(60);
    let mut turns: HashMap<String, Instant> =
        [("DP-1", now), ("DP-2", now), ("HDMI-1", later)].map(|(o, at)| (o.to_owned(), at)).into();
    let (mut missed, active) = (HashSet::new(), HashSet::from(["DP-1".to_owned(), "HDMI-1".to_owned()]));
    assert_eq!(take(&mut turns, &mut missed, now, &active), ["DP-1"]);
    assert_eq!(missed, HashSet::from(["DP-2".to_owned()]));
    assert_eq!(turns.keys().collect::<Vec<_>>(), ["HDMI-1"]);
    assert!(take(&mut turns, &mut missed, now, &active).is_empty(), "nothing is due twice");
    assert_eq!(missed.len(), 1);
    assert!(!back(&mut turns, &mut missed, |o| o != "DP-2"), "a hidden wallpaper never turns");
    assert_eq!((missed.len(), turns.len()), (1, 1));
    assert!(back(&mut turns, &mut missed, |_| true));
    assert!(missed.is_empty() && turns.contains_key("DP-2"), "the missed turn comes once it shows");
}

#[test]
fn big_folders_are_listed_within_a_budget_and_kept_until_they_change() {
    let dir = tempfile::tempdir().unwrap();
    for n in 0..SCAN + 50 {
        fs::write(dir.path().join(format!("{n:05}.png")), b"x").unwrap();
    }
    let found = files(dir.path(), &folder(json!({ "max_files": 3 })));
    assert!(found.len() == 3 && found.windows(2).all(|w| w[0] < w[1]), "{found:?}");
    let small = tempfile::tempdir().unwrap();
    fs::write(small.path().join("a.png"), b"x").unwrap();
    let prop = folder(json!({}));
    let named =
        || files(small.path(), &prop).iter().map(|p| p.file_name().unwrap().to_owned()).collect::<Vec<_>>();
    assert_eq!(named(), ["a.png"]);
    let at = fs::metadata(small.path()).unwrap().modified().unwrap();
    fs::write(small.path().join("b.png"), b"x").unwrap();
    let touch = |t| fs::File::open(small.path()).unwrap().set_modified(t).unwrap();
    touch(at);
    assert_eq!(named(), ["a.png"], "an unchanged folder is not read again");
    touch(at + Duration::from_secs(1));
    assert_eq!(named(), ["a.png", "b.png"], "a changed folder is read again");
}

#[test]
fn folders_read_their_subfolders_only_when_asked_and_cap_their_files() {
    let dir = tempfile::tempdir().unwrap();
    for path in [
        "a.png",
        "b.jpg",
        ".hidden.png",
        "notes.txt",
        "deep/c.png",
        "deep/deeper/d.png",
        "deep/deeper/far/e.png",
    ] {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }
    let names = |prop: &Property| {
        let found = files(dir.path(), prop);
        found
            .iter()
            .map(|p| p.strip_prefix(dir.path()).unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&folder(json!({}))), ["a.png", "b.jpg"]);
    assert_eq!(
        names(&folder(json!({ "recursive": 2 }))),
        ["a.png", "b.jpg", "deep/c.png", "deep/deeper/d.png"]
    );
    assert_eq!(names(&folder(json!({ "recursive": 1, "max_files": 2 }))), ["a.png", "b.jpg"]);
    assert!(pick(dir.path(), &folder(json!({ "pick": "sequence" }))).is_some());
}
