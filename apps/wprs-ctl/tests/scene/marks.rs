use super::authoring::{glow, shown};
use super::{fails, ok};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};

#[test]
fn layers_the_manifest_properties_and_the_canvas_are_authored_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    glow(dir);
    let layer = |args: &[&'static str]| [&["scene", "layer", "glow", "card"][..], args].concat();
    ok(
        dir,
        &layer(&[
            "transform",
            "--position",
            "100,-20",
            "--rotation",
            "-15",
            "--scale",
            "2,2",
            "--size",
            "300,200",
        ]),
    );
    assert_eq!(shown(dir, "/scene/layers/0/transform/position"), json!([100.0, -20.0]));
    ok(dir, &layer(&["blend", "screen"]));
    assert_eq!(shown(dir, "/scene/layers/0/blend"), "screen");
    assert_eq!(super::run(dir, &layer(&["blend", "glitter"])).status.code(), Some(2));
    assert!(fails(dir, &layer(&["transform", "--rotation", "900"])).contains("rotation"));
    ok(dir, &["scene", "add-layer", "glow", "particles", "--id", "dust"]);
    ok(
        dir,
        &[
            "scene", "layer", "glow", "dust", "instance", "--alpha", "0.5", "--count", "0.25", "--color",
            "#ffcc88",
        ],
    );
    ok(dir, &["scene", "layer", "glow", "dust", "instance", "--unset", "count"]);
    assert_eq!(shown(dir, "/scene/layers/2/instance"), json!({"alpha": 0.5, "color": "#ffcc88"}));
    assert!(fails(dir, &layer(&["instance", "--alpha", "1"])).contains("particles"));
    let meta = |args: &[&'static str]| [&["scene", "manifest", "glow"][..], args].concat();
    ok(dir, &meta(&["license", "CC-BY-4.0"]));
    ok(dir, &meta(&["authors", "Ana", "Luis"]));
    ok(dir, &meta(&["tags", "sea", "night"]));
    ok(dir, &meta(&["tags", "--remove", "night"]));
    ok(
        dir,
        &meta(&[
            "origin",
            "derived",
            "--derived-from",
            "Ana",
            "--derived-from",
            r#"{"author": "Luis", "license": "CC0-1.0"}"#,
        ]),
    );
    ok(dir, &meta(&["display", "--span", "all-outputs", "--accent", "#ff8800"]));
    assert!(!fails(dir, &meta(&["display", "--fit", "fit"])).is_empty());
    assert_eq!(shown(dir, "/manifest/tags"), json!(["sea"]));
    assert_eq!(shown(dir, "/manifest/origin/derived_from/1/license"), "CC0-1.0");
    assert_eq!(shown(dir, "/manifest/display/span"), "all-outputs");
    assert!(!fails(dir, &meta(&["license", "free as in beer"])).is_empty());
    assert!(!fails(dir, &meta(&["origin", "derived"])).is_empty());
    ok(dir, &meta(&["display", "--clear", "span"]));
    ok(dir, &["scene", "add-property", "glow", "slider", "--id", "speed"]);
    ok(
        dir,
        &[
            "scene",
            "property",
            "glow",
            "speed",
            "--min",
            "-1",
            "--max",
            "10",
            "--step",
            "0.5",
            "--default",
            "4",
            "--order",
            "0",
        ],
    );
    assert_eq!(shown(dir, "/manifest/properties/speed/min"), json!(-1.0));
    assert!(!fails(dir, &["scene", "property", "glow", "speed", "--default", "40"]).is_empty());
    ok(dir, &["scene", "canvas", "glow", "bloom", "--strength", "2", "--hdr", "2,1"]);
    ok(dir, &["scene", "canvas", "glow", "parallax", "--amount", "1.5", "--gyro", "true"]);
    assert_eq!(shown(dir, "/scene/canvas/bloom/hdr"), json!({"threshold": 2.0, "strength": 1.0}));
    assert!(fails(dir, &["scene", "canvas", "glow", "parallax", "--delay", "9"]).contains("delay"));
    ok(dir, &["scene", "canvas", "glow", "bloom", "--off"]);
    assert_eq!(shown(dir, "/scene/canvas/bloom"), Value::Null);
    RgbaImage::from_pixel(8, 8, Rgba([128, 128, 255, 255])).save(dir.join("sheet.png")).unwrap();
    assert_eq!(ok(dir, &["scene", "import", "glow", "sheet.png", "--atlas", "2,4"]), "sheet\n");
    ok(dir, &["scene", "import", "glow", "sheet.png", "--id", "bumps", "--linear"]);
    assert_eq!(shown(dir, "/scene/assets/sheet/atlas"), json!([2, 4]));
    assert_eq!(shown(dir, "/scene/assets/bumps/linear"), json!(true));
    assert_eq!(
        super::run(dir, &["scene", "import", "glow", "sheet.png", "--atlas", "2x4"]).status.code(),
        Some(2)
    );
    assert!(
        fails(dir, &["scene", "import", "glow", "sheet.png", "--id", "flat", "--atlas", "0,4"])
            .contains("256")
    );
}
