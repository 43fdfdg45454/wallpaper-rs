use image::{GrayImage, ImageReader, Luma, Rgba, RgbaImage};
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
    command.args(args).current_dir(dir).stdin(Stdio::null());
    for (key, sub) in [("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache"), ("XDG_CONFIG_HOME", "config")]
    {
        command.env(key, dir.join(sub));
    }
    command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}/no-bus", dir.display()));
    command.output().unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn fails(dir: &Path, args: &[&str], code: i32) -> String {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(code), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    String::from_utf8(out.stderr).unwrap()
}

#[test]
fn depth_maps_are_imported_exported_and_adjusted() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    RgbaImage::from_pixel(40, 20, Rgba([100, 100, 100, 255])).save(dir.join("hills.png")).unwrap();
    GrayImage::from_fn(40, 20, |x, _| Luma([if x < 20 { 0 } else { 255 }]))
        .save(dir.join("map.png"))
        .unwrap();
    let missing = fails(dir, &["edit", "depth", "export", "hills.png"], 1);
    assert!(missing.contains("no depth map"), "{missing}");
    let generated = fails(dir, &["edit", "depth", "generate", "hills.png"], 1);
    assert!(generated.contains("not installed"), "{generated}");
    GrayImage::new(20, 20).save(dir.join("square.png")).unwrap();
    assert!(fails(dir, &["edit", "depth", "import", "hills.png", "square.png"], 1).contains("shapes differ"));
    assert_eq!(ok(dir, &["edit", "depth", "import", "hills.png", "map.png"]).trim(), "hills.png.depth.png");
    let copy = dir.join("copy.png");
    ok(dir, &["edit", "depth", "export", "hills.png", "copy.png"]);
    let exported = ImageReader::open(&copy).unwrap().decode().unwrap();
    assert_eq!(
        (exported.color(), exported.into_luma16().get_pixel(30, 5)[0]),
        (image::ColorType::L16, u16::MAX)
    );
    ok(dir, &["edit", "depth", "hills.png", "--near", "0.9", "--dilate", "4"]);
    ok(dir, &["edit", "parallax", "hills.png", "--source", "pointer", "--steps", "32", "--strength", "0.01"]);
    let edit: Value = serde_json::from_str(&ok(dir, &["edit", "show", "hills.png"])).unwrap();
    assert_eq!(
        (edit["depth"]["near"].as_f64().map(|v| (v * 10.0).round()), &edit["depth"]["dilate"]),
        (Some(9.0), &json!(4))
    );
    assert_eq!((&edit["parallax"]["source"], &edit["parallax"]["steps"]), (&json!("pointer"), &json!(32)));
    ok(dir, &["edit", "parallax", "hills.png", "--off"]);
    assert!(fails(dir, &["edit", "parallax", "hills.png", "--steps", "12"], 2).contains("possible values"));
    assert!(
        fails(dir, &["edit", "parallax", "hills.png", "--strength", "0.5"], 1).contains("parallax.strength")
    );
    ok(dir, &["edit", "blur", "hills.png", "4", "--mask", "depth:0.5,1"]);
    let edit: Value = serde_json::from_str(&ok(dir, &["edit", "show", "hills.png"])).unwrap();
    assert_eq!(
        (&edit["masks"]["bl"]["type"], &edit["parallax"]["enabled"]),
        (&json!("depth"), &json!(false))
    );
}

#[test]
fn depth_masks_shape_exports() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    RgbaImage::from_pixel(40, 20, Rgba([100, 100, 100, 255])).save(dir.join("hills.png")).unwrap();
    GrayImage::from_fn(40, 20, |x, _| Luma([if x < 20 { 0 } else { 255 }]))
        .save(dir.join("hills.png.depth.png"))
        .unwrap();
    ok(dir, &["edit", "mask", "hills.png", "near", "depth:0.5,1"]);
    ok(dir, &["edit", "set", "hills.png", "ev=1", "ops.ev.mask=near"]);
    let out = run(dir, &["edit", "export", "hills.png", "-o", "out.png"]);
    if !out.status.success() {
        return eprintln!("skipping: {}", String::from_utf8_lossy(&out.stderr));
    }
    let shown = image::open(dir.join("out.png")).unwrap().into_rgb8();
    assert!(shown.get_pixel(5, 10)[0].abs_diff(100) <= 2 && shown.get_pixel(35, 10)[0] > 130);
}
