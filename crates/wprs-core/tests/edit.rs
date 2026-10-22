use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use wprs_core::edit::{self, Blend, Edit, Kind};

const FULL: &str = r##"{
  "version": 1,
  "source": { "name": "hills.jpg", "bytes": 8421337, "sha256": "9f2c", "pixels": [6000, 4000] },
  "geometry": { "crop": [0.0, 0.08, 1.0, 0.84], "rotate": 90, "straighten": -1.2, "flip": [false, true] },
  "ops": [
    { "id": "wb", "type": "white_balance", "temperature": 5600, "tint": 4 },
    { "id": "ev", "type": "exposure", "ev": 0.35, "opacity": 0.5 },
    { "id": "ct", "type": "contrast", "amount": 12 },
    { "id": "sa", "type": "saturation", "saturation": -8, "vibrance": 15 },
    { "id": "cu", "type": "curves", "rgb": [[0, 0], [0.25, 0.21], [0.75, 0.8], [1, 1]] },
    { "id": "bl", "type": "blur", "radius": 18, "mask": "back" },
    { "id": "ov", "type": "overlay", "color": "#1a2b6d", "blend": "soft_light", "mask": "sky" },
    { "id": "vi", "type": "vignette", "amount": -0.35, "midpoint": 0.5, "roundness": 0.0, "feather": 0.6, "enabled": false }
  ],
  "masks": {
    "back": { "type": "luminance", "range": [0.0, 0.35], "soft": 0.05 },
    "sky": { "type": "linear", "from": [0.5, 0.0], "to": [0.5, 0.45], "invert": true },
    "face": { "type": "brush", "strokes": [{ "radius": 0.04, "hardness": 0.6, "points": [[0.41, 0.33], [0.43, 0.35]] }] },
    "sun": { "type": "radial", "center": [0.7, 0.2], "radius": [0.1, 0.1], "feather": 0.5 }
  },
  "outputs": { "Maker Model 1234": { "connector": "DP-1", "crop": [0.0, 0.1, 0.5, 0.5625] } },
  "span": { "center": [0.5, 0.46], "zoom": 1.0 }
}"##;

#[test]
fn the_documented_example_round_trips() {
    let edit = Edit::parse(FULL).unwrap();
    assert_eq!(edit.ops.len(), 8);
    assert!(!edit.ops[7].enabled && edit.ops[0].enabled);
    assert_eq!(edit.ops[6].kind, Kind::Overlay { color: "#1a2b6d".into(), blend: Blend::SoftLight });
    assert_eq!(edit.crop_for("HDMI-A-1", Some("Maker Model 1234")), Some([0.0, 0.1, 0.5, 0.5625]));
    assert_eq!(edit.crop_for("DP-1", None), Some([0.0, 0.1, 0.5, 0.5625]));
    assert_eq!(edit.crop_for("DP-2", Some("Other")), None);
    assert_eq!(Edit::parse(&edit.to_json()).unwrap(), edit);
    assert!(edit.develops() && !Edit::default().develops());
}

#[test]
fn invalid_files_name_the_field() {
    let broken = |path: &str, value: serde_json::Value| {
        let mut doc: serde_json::Value = serde_json::from_str(FULL).unwrap();
        let mut at = &mut doc;
        let parts: Vec<&str> = path.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            at = match part.parse::<usize>() {
                Ok(i) => &mut at[i],
                Err(_) => &mut at[*part],
            };
        }
        at[parts[parts.len() - 1]] = value;
        Edit::from_value(doc).unwrap_err()
    };
    assert_eq!(broken("ops/1/ev", json!(7)), "ops[1].ev: 7 is out of range -5 to 5");
    assert_eq!(
        broken("ops/0/temperature", json!(900)),
        "ops[0].temperature: 900 is out of range 2000 to 12000"
    );
    assert_eq!(broken("ops/2/sharpness", json!(1)), "ops[2].sharpness: unknown field");
    assert_eq!(broken("geometry/rotate", json!(45)), "geometry.rotate: use 0, 90, 180 or 270");
    assert_eq!(broken("ops/5/mask", json!("nowhere")), "ops[5].mask: no mask is named nowhere");
    assert_eq!(broken("ops/1/id", json!("wb")), "ops[1].id: wb is used twice");
    assert_eq!(
        broken("ops/4/rgb", json!([[0.5, 0], [0.2, 1]])),
        "ops[4].rgb: x must grow from point to point"
    );
    assert_eq!(broken("ops/6/color", json!("blue")), "ops[6].color: use #rrggbb");
    assert_eq!(broken("span/zoom", json!(0.5)), "span.zoom: 0.5 is out of range 1 to 8");
    assert_eq!(broken("version", json!(2)), "version 2 is newer than 1");
    assert!(broken("masks/sky/type", json!("depth")).contains("unknown variant"));
    let crowded = json!({"version": 1, "ops": (0..25).map(|i| json!({"id": format!("e{i}"), "type": "exposure", "ev": 0})).collect::<Vec<_>>()});
    assert!(Edit::from_value(crowded).unwrap_err().contains("at most 24 ops"));
}

#[test]
fn set_takes_shorthands_and_dotted_paths() {
    let mut edit = Edit::default();
    edit.set("ev", "0.3").unwrap();
    edit.set("temperature", "5200").unwrap();
    edit.set("tint", "4").unwrap();
    edit.set("contrast", "-10").unwrap();
    let ids: Vec<&str> = edit.ops.iter().map(|op| op.id.as_str()).collect();
    assert_eq!(ids, ["ev", "wb", "ct"]);
    assert_eq!(edit.ops[1].kind, Kind::WhiteBalance { temperature: 5200.0, tint: 4.0 });
    edit.set("ops.ev.opacity", "0.25").unwrap();
    edit.set("ops.wb.enabled", "false").unwrap();
    edit.set("geometry.rotate", "180").unwrap();
    edit.set("geometry.crop", "0.1,0.1,0.5,0.5").unwrap();
    edit.set("span.zoom", "1.5").unwrap();
    assert_eq!((edit.ops[0].opacity, edit.ops[1].enabled), (Some(0.25), false));
    assert_eq!((edit.geometry.rotate, edit.geometry.crop), (180, Some([0.1, 0.1, 0.5, 0.5])));
    assert_eq!(edit.span.as_ref().map(|s| (s.zoom, s.center)), Some((1.5, [0.5, 0.5])));
    edit.set("geometry.crop", "null").unwrap();
    edit.set("span", "null").unwrap();
    assert_eq!((edit.geometry.crop, &edit.span), (None, &None));
    let before = edit.clone();
    assert_eq!(edit.set("ev", "9").unwrap_err(), "ops[0].ev: 9 is out of range -5 to 5");
    assert_eq!(edit.set("ops.ev.ev", "bright").unwrap_err(), "ops.ev.ev: bright is not a number");
    assert_eq!(edit.set("ops.nope.ev", "1").unwrap_err(), "ops.nope.ev: unknown setting");
    assert_eq!(edit.set("ops.ct.foo", "1").unwrap_err(), "ops[2].foo: unknown field");
    assert_eq!(edit, before);
}

#[test]
fn edits_live_beside_the_image_or_in_the_data_folder() {
    let dir = tempfile::tempdir().unwrap();
    let (image, edits) = (dir.path().join("hills.jpg"), dir.path().join("data/edits"));
    std::fs::write(&image, b"not really a picture").unwrap();
    assert_eq!(edit::load(&image, &edits).unwrap(), None);
    let mut edit = Edit::default();
    edit.set("ev", "1").unwrap();
    let saved = edit::save(&edit, &image, &edits).unwrap();
    assert_eq!(saved, dir.path().join("hills.jpg.wprs.json"));
    assert_eq!(std::fs::metadata(&saved).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(edit::load(&image, &edits).unwrap(), Some((saved.clone(), edit.clone())));
    let hidden = Path::new("/proc/self/status");
    let fallback = edit::save(&edit, hidden, &edits).unwrap();
    assert!(fallback.starts_with(&edits) && fallback.extension().is_some_and(|e| e == "json"));
    assert_eq!(std::fs::metadata(&edits).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(edit::load(hidden, &edits).unwrap().map(|(path, _)| path), Some(fallback.clone()));
    assert!(edit::remove(&image, &edits).unwrap() && !saved.exists());
    assert!(!edit::remove(&image, &edits).unwrap());
    std::fs::write(&saved, "{\"version\": 1, \"ops\": 3}").unwrap();
    assert!(edit::load(&image, &edits).unwrap_err().starts_with(&saved.display().to_string()));
    std::fs::write(&saved, " ".repeat(edit::MAX_BYTES + 1)).unwrap();
    assert!(edit::load(&image, &edits).unwrap_err().ends_with("is too large"));
    std::fs::remove_file(&saved).unwrap();
    let elsewhere = dir.path().join("elsewhere.json");
    std::os::unix::fs::symlink(&elsewhere, &saved).unwrap();
    let fallback = edit::save(&edit, &image, &edits).unwrap();
    assert!(fallback.starts_with(&edits) && !elsewhere.exists());
    std::fs::write(&elsewhere, Edit::default().to_json()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    edit::save(&edit, &image, &edits).unwrap();
    assert_eq!(edit::load(&image, &edits).unwrap(), Some((fallback, edit)));
}
