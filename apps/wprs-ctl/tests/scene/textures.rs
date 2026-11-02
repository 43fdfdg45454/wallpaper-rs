use super::*;

#[test]
fn a_picture_asset_is_shown_weighed_and_turned_into_ktx2() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "snow", "--from", "empty"]);
    RgbaImage::from_fn(40, 20, |x, y| Rgba([(x * 6) as u8, (y * 12) as u8, 80, 255]))
        .save(dir.join("sky.png"))
        .unwrap();
    assert_eq!(ok(dir, &["scene", "import", "snow", "sky.png"]), "sky\n");
    let plain = ok(dir, &["scene", "asset", "snow", "sky"]);
    assert!(plain.starts_with("sky: assets/sky.png, ") && plain.contains("40x20"), "{plain}");
    assert!(plain.contains("desktop bc7 3.1 KiB"), "{plain}");
    let looks = dir.join("looks");
    let args =
        ["scene", "asset", "snow", "sky", "--ktx2", "--dry-run", "--json", "--quality", "50", "--looks"];
    let trial: Value =
        serde_json::from_str(&ok(dir, &[&args[..], &[looks.to_str().unwrap()]].concat())).unwrap();
    assert_eq!((trial["size"].clone(), trial["quality"].clone()), (json!([40, 20]), json!(50)));
    assert!(looks.join("sky-uastc-50-bc7.png").is_file());
    assert_eq!(show(dir, "/scene/assets/sky/path"), json!("assets/sky.png"));
    assert_eq!(
        ok(dir, &["scene", "asset", "snow", "sky", "--ktx2", "--codec", "etc1s"]),
        "assets/sky.ktx2\n"
    );
    let packed: Value = serde_json::from_str(&ok(dir, &["scene", "asset", "snow", "sky", "--json"])).unwrap();
    assert_eq!((packed["codec"].clone(), packed["levels"].clone()), (json!("etc1s"), json!(6)));
    assert!(ok(dir, &["scene", "asset", "snow", "sky"]).contains("KTX2 etc1s with 6 levels"));
    ok(dir, &["scene", "validate", "snow"]);
    ok(dir, &["scene", "undo", "snow"]);
    assert_eq!(show(dir, "/scene/assets/sky/path"), json!("assets/sky.png"));
    assert!(fails(dir, &["scene", "asset", "snow", "sky", "--ktx2", "--quality", "0"]).contains("1 to 100"));
    assert!(fails(dir, &["scene", "asset", "snow", "moon"]).contains("no asset moon"));
    assert_eq!(
        run(dir, &["scene", "asset", "snow", "sky", "--ktx2", "--codec", "dxt5"]).status.code(),
        Some(2)
    );
    assert_eq!(run(dir, &["scene", "asset", "snow", "sky", "--dry-run"]).status.code(), Some(2));
    assert_eq!(run(dir, &["scene", "asset", "snow", "sky", "--ktx2", "--looks", "x"]).status.code(), Some(2));
}
