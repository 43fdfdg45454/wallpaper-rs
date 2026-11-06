use image::{Rgba, Rgba32FImage, RgbaImage};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;
use wprs_editor::{Project, packing};

fn project(root: &TempDir) -> Project {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
    let starter = starter("empty", "org.example.cubed", "Cubed", "en").unwrap();
    Project::create(&root.path().join("cubed"), starter).unwrap()
}

fn act(p: &mut Project, action: Value) -> anyhow::Result<()> {
    p.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

fn import(p: &mut Project, root: &TempDir, (id, name): (&str, &str), save: impl Fn(&std::path::Path)) {
    let file = root.path().join(name);
    save(&file);
    act(p, json!({ "import": { "file": file, "id": id } })).unwrap();
}

fn packed(p: &Project, id: &str) -> wprs_ktx2::Info {
    let path = p.doc["scene"]["assets"][id]["path"].as_str().unwrap().to_owned();
    assert!(path.ends_with(".ktx2"), "{path}");
    wprs_ktx2::info(&std::fs::read(p.dir.join(path)).unwrap()).unwrap()
}

#[test]
fn a_panorama_or_six_faces_become_a_cube_map_in_one_step() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let panorama = RgbaImage::from_fn(64, 32, |x, _| Rgba([(x * 4) as u8, 80, 160, 255]));
    import(&mut p, &root, ("sky", "sky.png"), |f| panorama.save(f).unwrap());
    let doc = p.doc.clone();
    let wanted = |extra: Value| {
        let mut action = json!({ "asset": "sky", "ktx2": true });
        action.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<wprs_editor::actions::AssetEdit>(action).unwrap()
    };
    let report = packing::trial(&p.dir, &doc, &wanted(json!({ "cube": true })), None).unwrap();
    assert_eq!((report["cube"].clone(), report["size"].clone()), (json!(true), json!([16, 16])));
    let classes = report["classes"].as_array().unwrap();
    assert!(classes.iter().all(|c| c["plain"] == (c["target"] != "rgba")), "{classes:?}");
    act(&mut p, json!({ "asset": { "asset": "sky", "ktx2": true, "cube": true } })).unwrap();
    let cube = packed(&p, "sky");
    assert_eq!((cube.cube, cube.picture(0)), (true, (64, 32)));
    let names = ["px", "nx", "py", "ny", "pz", "nz"];
    for (k, name) in names.iter().enumerate() {
        let face = RgbaImage::from_pixel(8, 8, Rgba([k as u8 * 40, 100, 100, 255]));
        import(&mut p, &root, (name, &format!("{name}.png")), |f| face.save(f).unwrap());
    }
    let faces: Vec<&str> = names.to_vec();
    act(&mut p, json!({ "asset": { "asset": "px", "ktx2": true, "faces": faces } })).unwrap();
    assert!(packed(&p, "px").cube);
    let five = &names[1..];
    let refused =
        act(&mut p, json!({ "asset": { "asset": "nx", "ktx2": true, "faces": five } })).unwrap_err();
    assert!(refused.to_string().contains("six square pictures"), "{refused}");
    let both = json!({ "asset": { "asset": "nx", "ktx2": true, "cube": true, "faces": odd_six(&names) } });
    assert!(act(&mut p, both).is_err(), "a panorama and six faces were taken at once");
    let wide = RgbaImage::from_pixel(8, 4, Rgba([1, 2, 3, 255]));
    import(&mut p, &root, ("wide", "wide.png"), |f| wide.save(f).unwrap());
    let odd = [names[1], names[2], names[3], names[4], names[5], "wide"];
    assert!(act(&mut p, json!({ "asset": { "asset": "nx", "ktx2": true, "faces": odd } })).is_err());
}

#[test]
fn an_hdr_picture_becomes_an_hdr_texture_that_keeps_its_light() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let bright = Rgba32FImage::from_fn(16, 8, |x, _| Rgba([if x < 8 { 5.0 } else { 0.3 }, 0.4, 0.2, 1.0]));
    import(&mut p, &root, ("sun", "sun.exr"), |f| bright.save(f).unwrap());
    assert_eq!(p.doc["scene"]["assets"]["sun"]["path"], json!("assets/sun.exr"));
    act(&mut p, json!({ "asset": { "asset": "sun", "ktx2": true } })).unwrap();
    let info = packed(&p, "sun");
    assert_eq!((info.hdr, info.codec, info.width), (true, "uastc-hdr", 16));
    let bytes = std::fs::read(p.dir.join(p.doc["scene"]["assets"]["sun"]["path"].as_str().unwrap())).unwrap();
    let half = wprs_ktx2::levels(&bytes, wprs_ktx2::Target::Half, 4096, false).unwrap();
    let red = f16(half.data[0], half.data[1]);
    assert!(red > 3.0, "the light above white was lost: {red}");
    let glow = Rgba32FImage::from_fn(32, 16, |x, _| Rgba([if x < 16 { 4.0 } else { 0.1 }, 0.2, 0.2, 1.0]));
    import(&mut p, &root, ("glow", "glow.exr"), |f| glow.save(f).unwrap());
    act(&mut p, json!({ "asset": { "asset": "glow", "ktx2": true, "cube": true } })).unwrap();
    let cube = packed(&p, "glow");
    assert!(cube.cube && cube.hdr, "an HDR panorama lost its light or its shape");
}

fn odd_six<'a>(names: &[&'a str]) -> Vec<&'a str> {
    vec![names[1], names[2], names[3], names[4], names[5], names[1]]
}

fn f16(low: u8, high: u8) -> f32 {
    let bits = u16::from_le_bytes([low, high]);
    let (exponent, mantissa) = (i32::from((bits >> 10) & 0x1f), f32::from(bits & 0x3ff));
    match exponent {
        0 => mantissa * 2f32.powi(-24),
        _ => (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15),
    }
}
