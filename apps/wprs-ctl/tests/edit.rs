use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
    command.args(args).current_dir(dir).stdin(Stdio::null()).env("XDG_DATA_HOME", dir.join("data"));
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

fn picture(dir: &Path) {
    RgbaImage::from_pixel(40, 20, Rgba([0, 255, 0, 255])).save(dir.join("hills.png")).unwrap();
}

#[test]
fn every_editor_action_edits_the_file_beside_the_image() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    picture(dir);
    let show = || serde_json::from_str::<Value>(&ok(dir, &["edit", "show", "hills.png"])).unwrap();
    assert_eq!((show()["ops"].clone(), show()["source"]["pixels"].clone()), (json!([]), json!([40, 20])));
    assert!(!dir.join("hills.png.wprs.json").exists());
    let edit = |args: &[&str]| ok(dir, &[&["edit", args[0], "hills.png"], &args[1..]].concat());
    edit(&["set", "ev=0.3", "contrast=10", "temperature=5200", "tint=4"]);
    edit(&["crop", "0.1,0.05,0.8,0.9", "--rotate", "90", "--straighten", "-1.5", "--flip", "h"]);
    edit(&["crop", "0,0.1,0.5,0.5625", "--output", "DP-1"]);
    edit(&["curve", "0,0", "0.25,0.2", "1,1", "--channel", "g"]);
    edit(&["blur", "12", "--mask", "linear:0.5,0,0.5,0.5"]);
    edit(&["vignette", "-0.4", "--feather", "0.6"]);
    edit(&["mask", "sky", "radial:0.5,0.3,0.4,0.2", "--invert"]);
    edit(&["overlay", "#ff8800", "--blend", "soft-light", "--opacity", "0.3", "--mask", "sky"]);
    edit(&["order", "ov", "ev"]);
    edit(&["span", "--zoom", "1.5", "--center", "0.4,0.5"]);
    edit(&["set", "ops.ov.opacity=0.2", "ops.ct.enabled=false"]);
    let saved = show();
    let ids: Vec<&str> =
        saved["ops"].as_array().unwrap().iter().map(|op| op["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["ov", "ev", "ct", "wb", "cu", "bl", "vi"]);
    assert_eq!(
        saved["geometry"],
        json!({"crop": [0.1, 0.05, 0.8, 0.9], "rotate": 90, "straighten": -1.5, "flip": [true, false]})
    );
    assert_eq!(saved["outputs"]["DP-1"], json!({"connector": "DP-1", "crop": [0.0, 0.1, 0.5, 0.5625]}));
    assert_eq!(
        saved["ops"][0],
        json!({"id": "ov", "enabled": true, "mask": "sky", "opacity": 0.2, "type": "overlay", "color": "#ff8800", "blend": "soft_light"})
    );
    assert_eq!(saved["ops"][4]["g"], json!([[0.0, 0.0], [0.25, 0.2], [1.0, 1.0]]));
    assert_eq!(
        saved["masks"]["bl"],
        json!({"type": "linear", "from": [0.5, 0.0], "to": [0.5, 0.5], "invert": false})
    );
    assert_eq!(
        (saved["masks"]["sky"]["feather"].clone(), saved["span"].clone()),
        (json!(0.5), json!({"center": [0.4, 0.5], "zoom": 1.5}))
    );
    let mode = std::fs::metadata(dir.join("hills.png.wprs.json")).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    assert!(
        fails(dir, &["edit", "set", "hills.png", "ev=9"], 1).contains("ops[1].ev: 9 is out of range -5 to 5")
    );
    assert!(fails(dir, &["edit", "set", "hills.png", "ev"], 2).contains("ev is not KEY=VALUE"));
    assert!(fails(dir, &["edit", "order", "hills.png", "nope"], 1).contains("no operation is named nope"));
    assert!(fails(dir, &["edit", "mask", "hills.png", "m", "square:1"], 2).contains("square is not linear"));
    assert!(fails(dir, &["edit", "show", "missing.png"], 1).contains("cannot find missing.png"));
    assert!(fails(dir, &["edit", "preview", "hills.png", "ev=1"], 1).contains("failed to connect"));
    edit(&["mask", "sky", "none"]);
    assert_eq!((show()["ops"][0].get("mask"), show()["masks"].get("sky")), (None, None));
    edit(&["reset", "ov", "bl"]);
    assert_eq!(show()["ops"].as_array().unwrap().len(), 5);
    edit(&["crop", "none", "--output", "DP-1"]);
    edit(&["crop", "none"]);
    let cleared = show();
    assert_eq!((&cleared["outputs"], &cleared["geometry"]["crop"]), (&json!({}), &json!(null)));
    std::fs::write(
        dir.join("new.json"),
        r#"{"version": 1, "ops": [{"id": "e", "type": "exposure", "ev": 2}]}"#,
    )
    .unwrap();
    edit(&["apply", "new.json"]);
    assert_eq!(show()["ops"], json!([{"id": "e", "enabled": true, "type": "exposure", "ev": 2.0}]));
    edit(&["reset"]);
    assert!(!dir.join("hills.png.wprs.json").exists() && dir.join("hills.png").exists());
}

#[test]
fn export_renders_the_edit_without_the_daemon() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    picture(dir);
    ok(dir, &["edit", "set", "hills.png", "ev=-1"]);
    let out = run(dir, &["edit", "export", "hills.png", "-o", "out.png"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() && std::env::var_os("WPRS_REQUIRE_GPU").is_none() && stderr.contains("GPU") {
        return eprintln!("skipping GPU test: {stderr}");
    }
    assert!(out.status.success(), "{stderr}");
    let png = image::open(dir.join("out.png")).unwrap().to_rgba8();
    assert_eq!(png.dimensions(), (40, 20));
    let Rgba([r, g, b, _]) = *png.get_pixel(20, 10);
    assert!(r <= 2 && g.abs_diff(188) <= 2 && b <= 2, "{:?}", png.get_pixel(20, 10));
    ok(dir, &["edit", "crop", "hills.png", "0,0,0.5,1"]);
    ok(dir, &["edit", "export", "hills.png", "-o", "small.JPG", "--quality", "80", "--size", "10x10"]);
    assert_eq!(image::open(dir.join("small.JPG")).unwrap().to_rgba8().dimensions(), (10, 10));
    let bands =
        RgbaImage::from_fn(40, 20, |x, _| Rgba([255 * u8::from(x < 13), 255 * u8::from(x < 27), 0, 255]));
    bands.save(dir.join("bands.png")).unwrap();
    ok(dir, &["edit", "export", "bands.png", "-o", "tall.png", "--size", "10x20"]);
    let tall = image::open(dir.join("tall.png")).unwrap().to_rgba8();
    assert!(tall.pixels().all(|p| p[0] < 8 && p[1] > 247), "the export is cropped, not stretched");
    assert!(fails(dir, &["edit", "export", "hills.png", "-o", "out.gif"], 2).contains(".png, .jpg or .jpeg"));
}
