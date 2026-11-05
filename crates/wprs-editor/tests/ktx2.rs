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
    assert_eq!((info.width, info.height, info.codec, info.levels), (70, 30, "uastc", 7));
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc["scene"]["assets"]["sky"]["path"], json!("assets/sky.png"));
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert_eq!(p.doc["scene"]["assets"]["sky"]["path"], json!(path));
    assert_eq!(wprs_editor::pictures::dimensions(p.dir.join(&path)).unwrap(), (70, 30));
}

#[test]
fn the_trial_reports_both_sizes_and_what_each_class_of_card_gets() {
    let root = tempfile::tempdir().unwrap();
    let p = project(&root);
    let looks = root.path().join("looks");
    let report = packing::trial(&p.dir, &p.doc, &wanted(Some("etc1s"), Some(40)), Some(&looks)).unwrap();
    assert_eq!(report["source"], json!([70, 30]));
    assert_eq!((report["size"].clone(), report.get("resized")), (json!([70, 30]), None));
    assert_eq!((report["codec"].as_str(), report["quality"].as_u64()), (Some("etc1s"), Some(40)));
    assert!(report["after"].as_u64().unwrap() > 0 && report["before"].as_u64().unwrap() > 0);
    let classes = report["classes"].as_array().unwrap();
    let names: Vec<&str> = classes.iter().map(|c| c["target"].as_str().unwrap()).collect();
    assert_eq!(names, ["bc7", "astc", "etc2", "rgba"]);
    assert!(classes[0]["gpu"].as_u64().unwrap() * 3 < classes[3]["gpu"].as_u64().unwrap());
    for class in classes {
        let look = image::open(class["look"].as_str().unwrap()).unwrap();
        assert_eq!((look.width(), look.height()), (70, 30));
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

#[test]
fn an_animated_picture_becomes_a_ktx2_array_that_keeps_its_timing() {
    use image::codecs::gif::GifEncoder;
    let root = tempfile::tempdir().unwrap();
    let p = project(&root);
    let file = p.dir.join("assets").join("blink.gif");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let mut gif = GifEncoder::new(std::fs::File::create(&file).unwrap());
    for c in [[255, 0, 0], [0, 255, 0], [0, 0, 255]] {
        let frame = RgbaImage::from_pixel(16, 8, Rgba([c[0], c[1], c[2], 255]));
        let delay = image::Delay::from_numer_denom_ms(200, 1);
        gif.encode_frame(image::Frame::from_parts(frame, 0, 0, delay)).unwrap();
    }
    drop(gif);
    let doc = json!({ "scene": { "assets": { "blink": { "path": "assets/blink.gif", "animated": true } } } });
    let wanted = AssetEdit { asset: "blink".into(), ..wanted(None, Some(70)) };
    let report = packing::trial(&p.dir, &doc, &wanted, None).unwrap();
    assert_eq!((report["frames"].as_u64(), report["size"].clone()), (Some(3), json!([16, 8])));
    let path = packing::store(&p.dir, &doc, &wanted).unwrap();
    let bytes = std::fs::read(p.dir.join(&path)).unwrap();
    let info = wprs_ktx2::info(&bytes).unwrap();
    assert_eq!((info.frames, info.picture(0), wprs_ktx2::quality(&bytes)), (3, (32, 16), Some(70)));
    assert_eq!(wprs_ktx2::durations(&bytes).unwrap(), vec![0.2; 3]);
    let glance = wprs_editor::pictures::glance(&p.dir.join(&path)).unwrap();
    assert_eq!((glance.frames, glance.cube, glance.hdr), (3, false, false));
    assert_eq!(wprs_editor::pictures::glance(&file), None);
}

#[test]
fn pictures_written_from_a_ktx2_source_keep_its_codec_and_quality() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path();
    let picture = RgbaImage::from_pixel(12, 6, Rgba([200, 100, 50, 255]));
    let options = wprs_ktx2::Options {
        codec: wprs_ktx2::Codec::Etc1s,
        quality: 33,
        srgb: true,
        mips: false,
        repeat: false,
    };
    std::fs::write(dir.join("old.ktx2"), wprs_ktx2::encode(&picture, &options).unwrap()).unwrap();
    picture.save(dir.join("old.png")).unwrap();
    let store = |like: Option<&str>| {
        wprs_editor::pictures::store(dir, ("masks", "m"), &picture, like.map(|l| dir.join(l)).as_deref())
            .unwrap()
    };
    let packed = store(Some("old.ktx2"));
    assert_eq!(packed, "masks/m-1.ktx2");
    let bytes = std::fs::read(dir.join(&packed)).unwrap();
    assert_eq!((wprs_ktx2::info(&bytes).unwrap().codec, wprs_ktx2::quality(&bytes)), ("etc1s", Some(33)));
    assert_eq!(
        (store(Some("old.png")), store(None), store(Some("gone.ktx2"))),
        ("masks/m-1.png".to_string(), "masks/m-2.png".to_string(), "masks/m-3.png".to_string())
    );
}
