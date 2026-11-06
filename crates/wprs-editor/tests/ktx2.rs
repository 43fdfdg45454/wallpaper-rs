use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::actions::{Action, AssetEdit};
use wprs_editor::starters::starter;
use wprs_editor::{Project, packing};

fn project(root: &TempDir) -> Project {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
    let mut p = Project::create(
        &root.path().join("packed"),
        starter("empty", "org.example.packed", "Packed", "en").unwrap(),
    )
    .unwrap();
    let file = root.path().join("sky.png");
    RgbaImage::from_fn(70, 30, |x, y| Rgba([(x * 3) as u8, (y * 8) as u8, 90, 255])).save(&file).unwrap();
    act(&mut p, json!({ "import": { "file": file, "id": "sky" } })).unwrap();
    p
}

fn act(p: &mut Project, action: Value) -> anyhow::Result<()> {
    p.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

fn wanted(codec: Option<&str>, quality: Option<u32>) -> AssetEdit {
    let (codec, cover, placeholder) = (codec.map(String::from), None, None);
    AssetEdit { asset: "sky".into(), ktx2: true, codec, quality, cover, placeholder, field: None }
}

#[test]
fn a_picture_becomes_a_ktx2_texture_as_one_undoable_step() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "asset": { "asset": "sky", "ktx2": true, "quality": 60 } })).unwrap();
    let path = p.doc["scene"]["assets"]["sky"]["path"].as_str().unwrap().to_string();
    assert_eq!(path, "assets/sky.ktx2");
    let info = wprs_ktx2::info(&std::fs::read(p.dir.join(&path)).unwrap()).unwrap();
    assert_eq!((info.width, info.height, info.codec, info.levels), (72, 32, "uastc", 7));
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc["scene"]["assets"]["sky"]["path"], json!("assets/sky.png"));
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert_eq!(p.doc["scene"]["assets"]["sky"]["path"], json!(path));
    assert_eq!(wprs_editor::pictures::dimensions(p.dir.join(&path)).unwrap(), (72, 32));
    let cache = root.path().join("shown");
    let shown = wprs_editor::pictures::shown(p.dir.join(&path), &cache).unwrap();
    assert!(shown.starts_with(&cache) && shown.extension().is_some_and(|e| e == "png"), "{shown:?}");
    assert_eq!(image::image_dimensions(&shown).unwrap(), (72, 32));
    assert_eq!(wprs_editor::pictures::shown(p.dir.join(&path), &cache).unwrap(), shown, "decoded again");
    let png = p.dir.join("assets/sky.png");
    assert_eq!(wprs_editor::pictures::shown(&png, &cache).unwrap(), png);
}

#[test]
fn the_decoded_pictures_keep_only_the_newest_ones() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "asset": { "asset": "sky", "ktx2": true } })).unwrap();
    let path = p.dir.join(p.doc["scene"]["assets"]["sky"]["path"].as_str().unwrap());
    let cache = root.path().join("shown");
    std::fs::create_dir_all(&cache).unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for k in 0..wprs_editor::pictures::SHOWN_KEPT + 4 {
        let f = std::fs::File::create(cache.join(format!("old{k}.png"))).unwrap();
        f.set_modified(old + std::time::Duration::from_secs(k as u64)).unwrap();
    }
    let shown = wprs_editor::pictures::shown(&path, &cache).unwrap();
    let left = std::fs::read_dir(&cache).unwrap().count();
    assert_eq!(left, wprs_editor::pictures::SHOWN_KEPT);
    assert!(shown.is_file() && !cache.join("old0.png").exists() && cache.join("old19.png").exists());
}

#[test]
fn the_trial_reports_both_sizes_and_what_each_class_of_card_gets() {
    let root = tempfile::tempdir().unwrap();
    let p = project(&root);
    let looks = root.path().join("looks");
    let report = packing::trial(&p.dir, &p.doc, &wanted(Some("etc1s"), Some(40)), Some(&looks)).unwrap();
    assert_eq!(report["source"], json!([70, 30]));
    assert_eq!((report["size"].clone(), report["resized"].clone()), (json!([72, 32]), json!(true)));
    assert_eq!((report["codec"].as_str(), report["quality"].as_u64()), (Some("etc1s"), Some(40)));
    assert!(report["after"].as_u64().unwrap() > 0 && report["before"].as_u64().unwrap() > 0);
    let classes = report["classes"].as_array().unwrap();
    let names: Vec<&str> = classes.iter().map(|c| c["target"].as_str().unwrap()).collect();
    assert_eq!(names, ["bc7", "astc", "etc2", "rgba"]);
    assert!(classes[0]["gpu"].as_u64().unwrap() * 3 < classes[3]["gpu"].as_u64().unwrap());
    for class in classes {
        let look = image::open(class["look"].as_str().unwrap()).unwrap();
        assert_eq!((look.width(), look.height()), (72, 32));
    }
    assert!(std::path::Path::new(report["original"].as_str().unwrap()).is_file());
    let quiet = packing::trial(&p.dir, &p.doc, &wanted(None, None), None).unwrap();
    assert_eq!((quiet["quality"].as_u64(), quiet["classes"][0]["look"].clone()), (Some(80), Value::Null));
    assert_eq!(p.doc["scene"]["assets"]["sky"]["path"], json!("assets/sky.png"));
}

#[test]
fn bad_conversions_are_refused_and_change_nothing() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    for bad in [
        json!({ "asset": { "asset": "moon", "ktx2": true } }),
        json!({ "asset": { "asset": "sky", "ktx2": true, "codec": "dxt5" } }),
        json!({ "asset": { "asset": "sky", "ktx2": true, "quality": 0 } }),
        json!({ "asset": { "asset": "sky", "ktx2": true, "quality": 101 } }),
        json!({ "asset": { "asset": "sky" } }),
    ] {
        assert!(act(&mut p, bad.clone()).is_err(), "{bad}");
    }
    assert!(serde_json::from_value::<Action>(json!({ "asset": { "asset": "sky", "size": 3 } })).is_err());
    assert_eq!(p.doc, before);
    act(&mut p, json!({ "asset": { "asset": "sky", "ktx2": true } })).unwrap();
    let again = act(&mut p, json!({ "asset": { "asset": "sky", "ktx2": true } })).unwrap_err();
    assert!(again.to_string().contains("not a still picture"), "{again}");
}
